use crate::allocator::linked_list::allocate_pages;
use crate::drivers::virtio_blk::VirtioBlk;
use crate::paging::PAGE_SHIFT;
use crate::{log_debug, log_info, log_warn};

const FAT32_SIGNATURE: [u8; 8] = [b'F', b'A', b'T', b'3', b'2', b' ', b' ', b' '];

const BYTES_PER_SECTOR_OFFSET: usize = 11;
const SECTORS_PER_CLUSTER_OFFSET: usize = 13;
const NUM_OF_RESERVED_CLUSTER_OFFSET: usize = 14;
const NUM_OF_FATS_OFFSET: usize = 16;
const FAT_SIZE_OFFSET: usize = 36;
const ROOT_CLUSTER_OFFSET: usize = 44;
const FAT32_SIGNATURE_OFFSET: usize = 82;

const FAT32_ATTRIBUTE_DIRECTORY: u8 = 0x10;
const FAT32_ATTRIBUTE_LONG_FILE_NAME: u8 = 0x0F;

pub struct Fat32 {
    base_lba: usize,
    lba_size: usize,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fat_sectors: u32,
    number_of_fats: u16,
    fat: usize,
    root_directory_list: usize,
}

pub struct FileInfo {
    entry_cluster: u32,
    file_size: u32,
}

#[repr(C)]
struct DirectoryEntry {
    name: [u8; 8],
    name_extension: [u8; 3],
    attribute: u8,
    reserved: [u8; 8],
    starting_cluster_number_high: u16,
    time_recorded: u16,
    date_recorded: u16,
    starting_cluster_number: u16,
    file_length: u32,
}

impl Fat32 {
    pub fn new(blk: &mut VirtioBlk, base_lba: usize, lba_size: usize) -> Result<Fat32, ()> {
        // BIOS Parameter Blockでファイルシステムを認識する
        let mut bpb_buffer: [u8; 512] = [0; 512];
        let bpb_address = &mut bpb_buffer as *mut _ as usize;
        blk.read(bpb_address, (base_lba * lba_size) as u64, 512)?;

        if unsafe { *((bpb_address + FAT32_SIGNATURE_OFFSET) as *const [u8; 8]) } != FAT32_SIGNATURE
        {
            log_warn!("Bad Signature");
            return Err(());
        }

        // BPBの読み取り
        // fat32の1セクタのサイズ
        // let bytes_per_sector = unsafe { *((bpb_address + BYTES_PER_SECTOR_OFFSET) as *const u16) };
        let bytes_per_sector = u16::from_le_bytes([
            bpb_buffer[BYTES_PER_SECTOR_OFFSET],
            bpb_buffer[BYTES_PER_SECTOR_OFFSET + 1],
        ]);
        // 1クラスタ（選択に使用する単位）が何セクタか
        let sectors_per_cluster =
            unsafe { *((bpb_address + SECTORS_PER_CLUSTER_OFFSET) as *const u8) };
        // 予約済みセクタ数
        let reserved_sectors = u16::from_le_bytes([
            bpb_buffer[NUM_OF_RESERVED_CLUSTER_OFFSET],
            bpb_buffer[NUM_OF_RESERVED_CLUSTER_OFFSET + 1],
        ]);
        // Fat (File Allocation Table) の数
        // バックアップのために複数存在する
        let number_of_fats = u16::from_le_bytes([
            bpb_buffer[NUM_OF_FATS_OFFSET],
            bpb_buffer[NUM_OF_FATS_OFFSET + 1],
        ]);
        // FATのセクタ数
        let fat_sectors = u32::from_le_bytes([
            bpb_buffer[FAT_SIZE_OFFSET],
            bpb_buffer[FAT_SIZE_OFFSET + 1],
            bpb_buffer[FAT_SIZE_OFFSET + 2],
            bpb_buffer[FAT_SIZE_OFFSET + 3],
        ]);
        // ルートディレクトリのクラスタの位置
        let root_cluster = u32::from_le_bytes([
            bpb_buffer[ROOT_CLUSTER_OFFSET],
            bpb_buffer[ROOT_CLUSTER_OFFSET + 1],
            bpb_buffer[ROOT_CLUSTER_OFFSET + 2],
            bpb_buffer[ROOT_CLUSTER_OFFSET + 3],
        ]);

        // FATの読み込み
        let fat_size = (fat_sectors as usize) * (bytes_per_sector as usize);
        let lba_aligned_fat_size = ((fat_size - 1) & (!(lba_size - 1))) + lba_size;
        // FATを読み込むためのメモリ領域を確保
        let fat = allocate_pages(
            (lba_aligned_fat_size >> PAGE_SHIFT) + 1,
            lba_size.ilog2() as usize,
        )
        .expect("Failed to allocate memory");
        let fat_address =
            ((base_lba * lba_size) as u64) + (reserved_sectors as u64) * (bytes_per_sector as u64);
        if blk
            .read(fat, fat_address, lba_aligned_fat_size as u64)
            .is_err()
        {
            // TODO: free_pages()を実装する
            // free_pages(fat, (lba_aligned_fat_size >> PAGE_SHIFT) + 1);
            return Err(());
        }

        // ルートディレクトリリストの読み込み
        let root_directory_pages =
            (((sectors_per_cluster as usize) * (bytes_per_sector as usize)) >> PAGE_SHIFT) + 1;
        let root_directory_list = allocate_pages(root_directory_pages, lba_size.ilog2() as usize)
            .expect("Failed to allocate memory");

        let fat32 = Fat32 {
            base_lba,
            lba_size,
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            fat_sectors,
            number_of_fats,
            fat,
            root_directory_list,
        };

        let root_sector = fat32.cluster_to_sector(root_cluster);
        if fat32
            .read_sectors(
                blk,
                root_directory_list,
                root_sector,
                sectors_per_cluster as u32,
            )
            .is_err()
        {
            // TODO: free_pages()
            return Err(());
        }
        Ok(fat32)
    }

    fn cluster_to_sector(&self, cluster: u32) -> u32 {
        (self.reserved_sectors as u32)
            + (self.number_of_fats as u32) * self.fat_sectors
            + (cluster - 2) * (self.sectors_per_cluster as u32) // クラスタ番号は2から始まるため
    }

    fn read_sectors(
        &self,
        blk: &mut VirtioBlk,
        buffer: usize,
        base_sector: u32,
        sectors: u32,
    ) -> Result<(), ()> {
        let block_address = ((self.base_lba * self.lba_size)
            + (base_sector as usize) * (self.bytes_per_sector as usize))
            as u64;
        let length = (sectors as u64) * (self.bytes_per_sector as u64);
        blk.read(buffer, block_address, length)
    }
    
    /// 現在のクラスタ番号からの次のクラスタ番号を取得する
    fn get_next_cluster(&self, cluster: u32) -> Option<u32> {
        let fat = unsafe {
            core::slice::from_raw_parts(
                self.fat as *const u32,
                (self.fat_sectors as usize * self.bytes_per_sector as usize) / size_of::<u32>(),
            )
        };
        let n = fat.get(cluster as usize)?;
        if (2..0x0FFFFFF8).contains(n) {
            Some(*n)
        } else {
            None
        }
    }

    fn get_file_name<'a>(entry: &DirectoryEntry, buffer: &'a mut [u8; 12]) -> Option<&'a mut str> {
        if entry.name[0] == 0x05 {
            // 0xE5は無効なファイルとして扱われる
            // ASCII文字以外では0xE5は使用される場合があり, 0x05に置き換えられている
            buffer[0] = 0xE5;
        } else {
            buffer[0] = entry.name[0];
        }
        let mut p = 0;
        for n in &entry.name[1..] {
            if *n == b' ' {
                continue;
            }
            p += 1;
            buffer[p] = *n;
        }
        p += 1;
        buffer[p] = b'.';
        for n in &entry.name_extension {
            if *n == b' ' {
                continue;
            }
            p += 1;
            buffer[p] = *n;
        }
        if buffer[p] == b'.' {
            buffer[p] = 0;
        } else {
            p += 1;
        }
        core::str::from_utf8_mut(&mut buffer[0..p]).ok()
    }

    pub fn list_files(&self) {
        let len = ((self.bytes_per_sector as usize) * (self.sectors_per_cluster as usize))
            / size_of::<DirectoryEntry>();
        let entries = unsafe {
            core::slice::from_raw_parts(self.root_directory_list as *mut DirectoryEntry, len)
        };
        for e in entries {
            if (e.attribute & 0x3F) == FAT32_ATTRIBUTE_LONG_FILE_NAME {
                continue;
            }
            if (e.attribute & FAT32_ATTRIBUTE_DIRECTORY) != 0 {
                continue;
            }
            if e.name[0] == 0 {
                continue;
            } else if e.name[0] == 0xE5 {
                log_debug!("continued with 0xE5");
                continue;
            }
            let mut buffer = [0u8; 12];
            if let Some(file_name) = Self::get_file_name(e, &mut buffer) {
                let file_size = e.file_length;
                log_info!("{}: File Size: {:#X}", file_name, file_size);
            }
        }
    }

    pub fn search_file(&self, target_name: &str) -> Option<FileInfo> {
        let len = ((self.bytes_per_sector as usize) * self.sectors_per_cluster as usize)
            / size_of::<DirectoryEntry>();
        let entries = unsafe {
            core::slice::from_raw_parts(self.root_directory_list as *const DirectoryEntry, len)
        };
        assert_eq!(size_of::<DirectoryEntry>(), 32);

        for e in entries {
            if (e.attribute & 0x3F) == FAT32_ATTRIBUTE_LONG_FILE_NAME {
                continue;
            }
            if (e.attribute & FAT32_ATTRIBUTE_DIRECTORY) != 0 {
                continue;
            }
            if e.name[0] == 0 {
                break;
            } else if e.name[0] == 0xE5 {
                continue;
            }
            let mut buffer = [0u8; 12];
            if let Some(file_name) = Self::get_file_name(e, &mut buffer) {
                let mut is_same = file_name == target_name;
                if !is_same {
                    file_name.make_ascii_lowercase();
                    is_same = file_name == target_name;
                }
                if is_same {
                    let entry_cluster = ((e.starting_cluster_number_high as u32) << 16)
                        | (e.starting_cluster_number as u32);
                    let file_size = e.file_length;
                    return Some(FileInfo {
                        entry_cluster,
                        file_size,
                    });
                }
            }
        }
        None
    }
    
    pub fn read(
        &self,
        file_info: &FileInfo,
        blk: &mut VirtioBlk,
        buffer_address: usize,
        offset: usize,
        mut length: usize,
    ) -> Result<usize, ()> {
        if offset + length > file_info.file_size as usize {
            // offsetがファイルサイズを超えていた場合は何も読み込まない
            if offset >= file_info.file_size as usize {
                return Ok(0);
            }
            // 読み込む長さがファイルを超えてしまう場合はファイルの終端に合わせる
            length = (file_info.file_size as usize) - offset;
        }
        
        macro_rules! next_cluster {
            ($c:expr) => {
                match self.get_next_cluster($c) {
                    Some(n) => n,
                    None => {
                        log_warn!("Failed to get next cluster");
                        return Err(());
                    }
                }
            };
        }
        
        let bytes_per_cluster = self.sectors_per_cluster as usize * self.bytes_per_sector as usize;
        let clusters_to_skip = offset / bytes_per_cluster;
        let mut data_offset = offset - clusters_to_skip * bytes_per_cluster;
        let mut reading_cluster = file_info.entry_cluster;
        let mut buffer_pointer = 0usize;
        
        for _ in 0..clusters_to_skip {
            reading_cluster = next_cluster!(reading_cluster);
        }
        
        // クラスタごとの読み込みをするループ
        loop {
            let mut sectors = 0;
            let mut read_bytes = 0; 
            let mut sector_offset = 0;
            let first_cluster = reading_cluster;
            let mut data_offset_backup = data_offset;

            // 連続して読み込めるセクタ数を求めるループ
            loop {
                // 読み飛ばすセクタ数の計算
                if data_offset > (self.bytes_per_sector as usize) {
                    sector_offset = (data_offset / self.bytes_per_sector as usize) as u32;
                    data_offset -= (sector_offset as usize) * (self.bytes_per_sector as usize);
                    data_offset_backup = data_offset;
                }
                
                // 読み込むサイズが1クラスタ分に満たない場合
                if (length - read_bytes + data_offset) <= bytes_per_cluster {
                    sectors += (1
                        + ((length - read_bytes + data_offset).max(1) - 1)
                            / self.bytes_per_sector as usize) as u32;
                    read_bytes += length - read_bytes;
                    break;
                }
                
                // 1クラスタ分読み込む
                sectors += self.sectors_per_cluster as u32;
                read_bytes += bytes_per_cluster - data_offset;

                let next_cluster = next_cluster!(reading_cluster);
                if next_cluster != reading_cluster + 1 {
                    // クラスタが連続していない
                    break;
                }
                data_offset = 0;
                reading_cluster = next_cluster;
            }
            
            data_offset = data_offset_backup;
            
            let aligned_buffer_size = (((sectors as usize) * (self.bytes_per_sector as usize))
                & (!(self.lba_size - 1)))
                + self.lba_size;
            // Offsetがある場合は先に別のページに読み込んで, あとからoffsetをつけてコピーする
            let buffer = if data_offset != 0 {
                allocate_pages((aligned_buffer_size >> PAGE_SHIFT) + 1, 0).or(Err(()))?
            } else {
                buffer_address + buffer_pointer
            };

            let sector = self.cluster_to_sector(first_cluster) + sector_offset;
            if self.read_sectors(blk, buffer, sector, sectors).is_err() {
                // TODO: 
                // if data_offset != 0 {
                //     free_pages(buffer, (aligned_buffer_size >> PAGE_SHIFT) + 1);
                // }
                return Err(());
            };
            if data_offset != 0 {
                assert_eq!(buffer_pointer, 0);
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        (buffer + data_offset) as *const u8,
                        buffer_address as *mut u8,
                        read_bytes,
                    )
                };
                // TODO: 
                // free_pages(buffer, (aligned_buffer_size >> PAGE_SHIFT) + 1);
                data_offset = 0;
            }
            buffer_pointer += read_bytes;
            if length == buffer_pointer {
                break;
            }
            reading_cluster = next_cluster!(reading_cluster)
        }
        Ok(buffer_pointer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_entry(
        name: [u8; 8],
        extension: [u8; 3],
        attribute: u8,
        cluster_high: u16,
        cluster_low: u16,
        file_length: u32,
    ) -> DirectoryEntry {
        DirectoryEntry {
            name,
            name_extension: extension,
            attribute,
            reserved: [0; 8],
            starting_cluster_number_high: cluster_high,
            time_recorded: 0,
            date_recorded: 0,
            starting_cluster_number: cluster_low,
            file_length,
        }
    }

    fn make_fat32_for_test(
        bytes_per_sector: u16,
        sectors_per_cluster: u8,
        reserved_sectors: u16,
        fat_sectors: u32,
        number_of_fats: u16,
        fat: usize,
        root_directory_list: usize,
    ) -> Fat32 {
        Fat32 {
            base_lba: 0,
            lba_size: 512,
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            fat_sectors,
            number_of_fats,
            fat,
            root_directory_list,
        }
    }

    #[test_case]
    fn fat32_get_file_name_handles_with_and_without_extension() {
        // 8.3形式の名前変換を確認, 拡張子ありとなしの両方を検証
        let with_ext = build_entry(*b"MIN1    ", *b"ELF", 0, 0, 0, 0);
        let mut with_ext_buf = [0u8; 12];
        let with_ext_name = Fat32::get_file_name(&with_ext, &mut with_ext_buf).unwrap();
        assert_eq!(with_ext_name, "MIN1.ELF");

        let without_ext = build_entry(*b"README  ", *b"   ", 0, 0, 0, 0);
        let mut without_ext_buf = [0u8; 12];
        let without_ext_name = Fat32::get_file_name(&without_ext, &mut without_ext_buf).unwrap();
        assert_eq!(without_ext_name, "README");
    }

    #[test_case]
    fn fat32_search_file_ignores_non_files_and_matches_case_insensitive() {
        // 検索時のフィルタ条件を確認, LFNとディレクトリ除外と大文字小文字差分を検証
        let mut entries = [
            build_entry(*b"LONGNAME", *b"LNG", FAT32_ATTRIBUTE_LONG_FILE_NAME, 0, 0, 0),
            build_entry(*b"BOOT    ", *b"   ", FAT32_ATTRIBUTE_DIRECTORY, 0, 2, 0),
            build_entry(*b"MIN1    ", *b"ELF", 0, 0xABCD, 0x1234, 0x2500),
            build_entry([0; 8], [0; 3], 0, 0, 0, 0),
        ];

        let fat32 = make_fat32_for_test(
            (entries.len() * size_of::<DirectoryEntry>()) as u16,
            1,
            32,
            100,
            2,
            0,
            entries.as_mut_ptr() as usize,
        );

        let file = fat32.search_file("min1.elf").expect("file should be found");
        assert_eq!(file.entry_cluster, 0xABCD_1234);
        assert_eq!(file.file_size, 0x2500);
    }

    #[test_case]
    fn fat32_search_file_stops_at_end_marker() {
        // 終端マーカーで探索停止することを確認, 終端以降の有効エントリを無視することを検証
        let mut entries = [
            build_entry([0; 8], [0; 3], 0, 0, 0, 0),
            build_entry(*b"MIN1    ", *b"ELF", 0, 0, 2, 0x1000),
        ];

        let fat32 = make_fat32_for_test(
            (entries.len() * size_of::<DirectoryEntry>()) as u16,
            1,
            32,
            100,
            2,
            0,
            entries.as_mut_ptr() as usize,
        );

        assert!(fat32.search_file("MIN1.ELF").is_none());
    }

    #[test_case]
    fn fat32_get_next_cluster_respects_fat_boundaries() {
        // FATチェーン追跡の境界を確認, 有効値とEOCと範囲外の扱いを検証
        let mut fat = [0u32; 8];
        fat[2] = 3;
        fat[3] = 0x0FFF_FFF8;
        fat[4] = 1;

        let fat32 = make_fat32_for_test(32, 1, 32, 1, 2, fat.as_mut_ptr() as usize, 0);

        assert_eq!(fat32.get_next_cluster(2), Some(3));
        assert_eq!(fat32.get_next_cluster(3), None);
        assert_eq!(fat32.get_next_cluster(4), None);
        assert_eq!(fat32.get_next_cluster(100), None);
    }

    #[test_case]
    fn fat32_cluster_to_sector_uses_reserved_and_fat_regions() {
        // クラスタからセクタへの計算を確認, 予約領域とFAT領域の加算を検証
        let fat32 = make_fat32_for_test(512, 8, 32, 100, 2, 0, 0);

        assert_eq!(fat32.cluster_to_sector(2), 232);
        assert_eq!(fat32.cluster_to_sector(5), 256);
    }
}
