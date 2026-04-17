use crate::allocator::linked_list::allocate_pages;
use crate::drivers::virtio_blk::VirtioBlk;
use crate::log_warn;
use crate::paging::PAGE_SHIFT;

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
        if fat32.read_sectors(blk, root_directory_list, root_sector, sectors_per_cluster as u32).is_err() {
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
}
