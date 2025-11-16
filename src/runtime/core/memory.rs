/// Linear memory management for WASM execution
/// Implements 64KB pages with bounds checking and safe read/write operations
const PAGE_SIZE: usize = 65536; // 64KB

#[derive(Debug, Clone)]
pub struct LinearMemory {
    pages: Vec<Vec<u8>>,
    initial: u32,
    max: Option<u32>,
}

impl LinearMemory {
    /// Create new linear memory with given initial and max pages
    pub fn new(initial: u32, max: Option<u32>) -> Result<Self, String> {
        if let Some(max_pages) = max {
            if initial > max_pages {
                return Err(format!(
                    "Initial pages ({initial}) exceeds max pages ({max_pages})"
                ));
            }
        }

        let mut pages = Vec::with_capacity(initial as usize);
        for _ in 0..initial {
            pages.push(vec![0u8; PAGE_SIZE]);
        }

        Ok(LinearMemory {
            pages,
            initial,
            max,
        })
    }

    /// Get current size in pages
    pub fn size(&self) -> u32 {
        self.pages.len() as u32
    }

    /// Get current size in bytes
    pub fn size_bytes(&self) -> usize {
        self.pages.len() * PAGE_SIZE
    }

    /// Grow memory by given number of pages, return old size in pages
    pub fn grow(&mut self, pages: u32) -> Result<u32, String> {
        let current_size = self.size();

        // Check max limit
        if let Some(max_pages) = self.max {
            if current_size + pages > max_pages {
                return Err(format!(
                    "Cannot grow memory: current {current_size} pages + {pages} pages > max {max_pages} pages"
                ));
            }
        }

        for _ in 0..pages {
            self.pages.push(vec![0u8; PAGE_SIZE]);
        }

        Ok(current_size)
    }

    /// Read a single byte at given address
    pub fn read_u8(&self, addr: usize) -> Result<u8, String> {
        if addr >= self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: read at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let page_idx = addr / PAGE_SIZE;
        let offset = addr % PAGE_SIZE;
        Ok(self.pages[page_idx][offset])
    }

    /// Write a single byte at given address
    pub fn write_u8(&mut self, addr: usize, value: u8) -> Result<(), String> {
        if addr >= self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: write at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let page_idx = addr / PAGE_SIZE;
        let offset = addr % PAGE_SIZE;
        self.pages[page_idx][offset] = value;
        Ok(())
    }

    /// Read i32 (4 bytes, little-endian)
    pub fn read_i32(&self, addr: usize) -> Result<i32, String> {
        if addr + 4 > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: read i32 at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let bytes = [
            self.read_u8(addr)?,
            self.read_u8(addr + 1)?,
            self.read_u8(addr + 2)?,
            self.read_u8(addr + 3)?,
        ];
        Ok(i32::from_le_bytes(bytes))
    }

    /// Write i32 (4 bytes, little-endian)
    pub fn write_i32(&mut self, addr: usize, value: i32) -> Result<(), String> {
        if addr + 4 > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: write i32 at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let bytes = value.to_le_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            self.write_u8(addr + i, b)?;
        }
        Ok(())
    }

    /// Read i64 (8 bytes, little-endian)
    pub fn read_i64(&self, addr: usize) -> Result<i64, String> {
        if addr + 8 > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: read i64 at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let bytes = [
            self.read_u8(addr)?,
            self.read_u8(addr + 1)?,
            self.read_u8(addr + 2)?,
            self.read_u8(addr + 3)?,
            self.read_u8(addr + 4)?,
            self.read_u8(addr + 5)?,
            self.read_u8(addr + 6)?,
            self.read_u8(addr + 7)?,
        ];
        Ok(i64::from_le_bytes(bytes))
    }

    /// Write i64 (8 bytes, little-endian)
    pub fn write_i64(&mut self, addr: usize, value: i64) -> Result<(), String> {
        if addr + 8 > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: write i64 at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let bytes = value.to_le_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            self.write_u8(addr + i, b)?;
        }
        Ok(())
    }

    /// Read f32 (4 bytes, little-endian)
    pub fn read_f32(&self, addr: usize) -> Result<f32, String> {
        if addr + 4 > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: read f32 at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let bytes = [
            self.read_u8(addr)?,
            self.read_u8(addr + 1)?,
            self.read_u8(addr + 2)?,
            self.read_u8(addr + 3)?,
        ];
        Ok(f32::from_le_bytes(bytes))
    }

    /// Write f32 (4 bytes, little-endian)
    pub fn write_f32(&mut self, addr: usize, value: f32) -> Result<(), String> {
        if addr + 4 > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: write f32 at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let bytes = value.to_le_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            self.write_u8(addr + i, b)?;
        }
        Ok(())
    }

    /// Read f64 (8 bytes, little-endian)
    pub fn read_f64(&self, addr: usize) -> Result<f64, String> {
        if addr + 8 > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: read f64 at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let bytes = [
            self.read_u8(addr)?,
            self.read_u8(addr + 1)?,
            self.read_u8(addr + 2)?,
            self.read_u8(addr + 3)?,
            self.read_u8(addr + 4)?,
            self.read_u8(addr + 5)?,
            self.read_u8(addr + 6)?,
            self.read_u8(addr + 7)?,
        ];
        Ok(f64::from_le_bytes(bytes))
    }

    /// Write f64 (8 bytes, little-endian)
    pub fn write_f64(&mut self, addr: usize, value: f64) -> Result<(), String> {
        if addr + 8 > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: write f64 at {} (size: {} bytes)",
                addr,
                self.size_bytes()
            ));
        }

        let bytes = value.to_le_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            self.write_u8(addr + i, b)?;
        }
        Ok(())
    }

    /// Read a slice of bytes
    pub fn read_bytes(&self, addr: usize, len: usize) -> Result<Vec<u8>, String> {
        if addr + len > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: read {} bytes at {} (size: {} bytes)",
                len,
                addr,
                self.size_bytes()
            ));
        }

        let mut result = Vec::with_capacity(len);
        for i in 0..len {
            result.push(self.read_u8(addr + i)?);
        }
        Ok(result)
    }

    /// Write a slice of bytes
    pub fn write_bytes(&mut self, addr: usize, data: &[u8]) -> Result<(), String> {
        if addr + data.len() > self.size_bytes() {
            return Err(format!(
                "Memory access out of bounds: write {} bytes at {} (size: {} bytes)",
                data.len(),
                addr,
                self.size_bytes()
            ));
        }

        for (i, &byte) in data.iter().enumerate() {
            self.write_u8(addr + i, byte)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_allocation() {
        let mem = LinearMemory::new(2, Some(4)).unwrap();
        assert_eq!(mem.size(), 2);
        assert_eq!(mem.size_bytes(), 2 * PAGE_SIZE);
    }

    #[test]
    fn test_memory_allocation_exceeds_max() {
        let result = LinearMemory::new(5, Some(3));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Initial pages (5) exceeds max pages (3)"));
    }

    #[test]
    fn test_memory_grow() {
        let mut mem = LinearMemory::new(1, Some(3)).unwrap();
        assert_eq!(mem.size(), 1);

        let old_size = mem.grow(2).unwrap();
        assert_eq!(old_size, 1);
        assert_eq!(mem.size(), 3);
    }

    #[test]
    fn test_memory_grow_exceeds_max() {
        let mut mem = LinearMemory::new(2, Some(3)).unwrap();
        let result = mem.grow(2);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_read_u8() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_u8(100, 42).unwrap();
        assert_eq!(mem.read_u8(100).unwrap(), 42);
    }

    #[test]
    fn test_write_read_i32() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let value = -12345i32;
        mem.write_i32(1000, value).unwrap();
        assert_eq!(mem.read_i32(1000).unwrap(), value);
    }

    #[test]
    fn test_write_read_i64() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let value = -9876543210i64;
        mem.write_i64(2000, value).unwrap();
        assert_eq!(mem.read_i64(2000).unwrap(), value);
    }

    #[test]
    fn test_write_read_f32() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let value = std::f32::consts::PI;
        mem.write_f32(3000, value).unwrap();
        let read_val = mem.read_f32(3000).unwrap();
        assert!((read_val - value).abs() < 0.00001);
    }

    #[test]
    fn test_write_read_f64() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let value = std::f64::consts::E;
        mem.write_f64(4000, value).unwrap();
        let read_val = mem.read_f64(4000).unwrap();
        assert!((read_val - value).abs() < 0.0000000001);
    }

    #[test]
    fn test_bounds_checking_read() {
        let mem = LinearMemory::new(1, None).unwrap();
        let result = mem.read_u8(PAGE_SIZE + 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_bounds_checking_write() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let result = mem.write_u8(PAGE_SIZE + 100, 42);
        assert!(result.is_err());
    }

    #[test]
    fn test_bounds_checking_i32_read() {
        let mem = LinearMemory::new(1, None).unwrap();
        let result = mem.read_i32(PAGE_SIZE - 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_bounds_checking_i32_write() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let result = mem.write_i32(PAGE_SIZE - 2, 42);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_write_bytes() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let data = b"Hello, WASM!";
        mem.write_bytes(5000, data).unwrap();

        let read_data = mem.read_bytes(5000, data.len()).unwrap();
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_multiple_pages() {
        let mut mem = LinearMemory::new(1, Some(3)).unwrap();

        // Write to end of first page
        mem.write_i32(PAGE_SIZE - 4, 0xDEADBEEFu32 as i32).unwrap();
        assert_eq!(mem.read_i32(PAGE_SIZE - 4).unwrap(), 0xDEADBEEFu32 as i32);

        // Grow and write to second page
        mem.grow(1).unwrap();
        mem.write_i32(PAGE_SIZE, 0xCAFEBABEu32 as i32).unwrap();
        assert_eq!(mem.read_i32(PAGE_SIZE).unwrap(), 0xCAFEBABEu32 as i32);

        // Verify first page still intact
        assert_eq!(mem.read_i32(PAGE_SIZE - 4).unwrap(), 0xDEADBEEFu32 as i32);
    }
}
