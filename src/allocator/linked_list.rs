use core::ptr;

#[derive(Debug)]
pub struct ListNode {
    size: usize,
    pub next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

#[derive(Debug)]
pub struct LinkedListAllocator {
    pub head: ListNode,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    /// 空きリストの先頭にノードを追加する
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        assert_eq!(align_up(addr, core::mem::align_of::<ListNode>()), addr);
        assert!(size >= core::mem::size_of::<ListNode>());

        let mut new_node = ListNode::new(size);
        new_node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        unsafe {
            node_ptr.write(new_node);
            self.head.next = Some(&mut *node_ptr);
        }
    }

    /// (addr, size)からListNodeとして表現可能なfree regionを1個作る
    /// 無理ならnextをそのまま返す
    fn region_if_representable(
        addr: usize,
        size: usize,
        next: Option<&'static mut ListNode>,
    ) -> Option<&'static mut ListNode> {
        let aligned_addr = align_up(addr, core::mem::align_of::<ListNode>());
        let adjusted_size = size.saturating_sub(aligned_addr - addr);

        if adjusted_size >= core::mem::size_of::<ListNode>() {
            let mut node = ListNode::new(adjusted_size);
            node.next = next;
            let node_ptr = aligned_addr as *mut ListNode;
            unsafe {
                node_ptr.write(node);
                Some(&mut *node_ptr)
            }
        } else {
            next
        }
    }

    /// 使用可能なregionを探してリストから外す
    /// ノードと開始アドレスを返す
    fn find_and_take_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        let mut current = &mut self.head;
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::validate_region(&region, size, align) {
                // 適切なノードが見つかったらリストから外す
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                current = current.next.as_mut().unwrap();
            }
        }
        None
    }

    /// 使用可能なregionか判定する
    /// アライメントを行う
    fn validate_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            return Err(());
        }

        // 残りの領域がピッタリ収まっているならOK
        // 残った部分がノードを格納できるほどのサイズが無い場合はErr
        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < core::mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    pub unsafe fn alloc(&mut self, size: usize, align: usize) -> *mut u8 {
        if let Some((region, alloc_start)) = self.find_and_take_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                unsafe {
                    self.add_free_region(alloc_end, excess_size);
                }
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }
    
    pub unsafe fn dealloc(&mut self, ptr: *mut u8, size: usize) {
        unsafe {
            self.add_free_region(ptr as usize, size);
        }
    }

    /// 指定した範囲を使用済みとして空きリストから除外する
    pub fn reserve(&mut self, addr: usize, size: usize) {
        let reserve_end = addr.checked_add(size).expect("overflow");
        let mut current = &mut self.head;

        while let Some(region) = current.next.take() {
            let region_start = region.start_addr();
            let region_end = region.end_addr();
            let next = region.next.take();

            if reserve_end <= region_start || region_end <= addr {
                region.next = next;
                current.next = Some(region);
                current = current.next.as_mut().unwrap();
                continue;
            }

            current.next = next;

            // 重なっている部分の大きさを前半と後半に分けて考える
            let prefix_size = addr.saturating_sub(region_start);
            let suffix_start = reserve_end.max(region_start);
            let suffix_size = region_end.saturating_sub(suffix_start);
            let next = Self::region_if_representable(suffix_start, suffix_size, current.next.take());
            current.next = Self::region_if_representable(region_start, prefix_size, next);
        }
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    addr.next_multiple_of(align)
}
