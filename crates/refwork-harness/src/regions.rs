use std::fmt;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;

use refwork_emu::{RegionBuffers, FB_BYTES};

pub const PAGE_SIZE: usize = 4096;
pub const WRAM_SIZE: usize = 0x20000;
pub const VRAM_SIZE: usize = 0x10000;
pub const META_SIZE: usize = crate::meta::META_SIZE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionError {
    Empty {
        name: &'static str,
    },
    NotPageMultiple {
        name: &'static str,
        len: usize,
    },
    InvalidSramLen {
        len: usize,
    },
    Map {
        name: &'static str,
        len: usize,
        errno: i32,
    },
    Published {
        name: &'static str,
    },
    WrongSize {
        name: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for RegionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegionError::Empty { name } => write!(f, "region `{name}` has zero length"),
            RegionError::NotPageMultiple { name, len } => {
                write!(f, "region `{name}` length {len} is not a page multiple")
            }
            RegionError::InvalidSramLen { len } => {
                write!(f, "sram logical length {len} is not supported")
            }
            RegionError::Map { name, len, errno } => write!(
                f,
                "mmap for region `{name}` length {len} failed with errno {errno}"
            ),
            RegionError::Published { name } => {
                write!(f, "region `{name}` is already published")
            }
            RegionError::WrongSize {
                name,
                expected,
                actual,
            } => write!(
                f,
                "region `{name}` length {actual} does not match expected length {expected}"
            ),
        }
    }
}

impl std::error::Error for RegionError {}

pub struct PublishedRegion {
    name: &'static str,
    ptr: NonNull<u8>,
    len: usize,
    published: bool,
}

impl PublishedRegion {
    /// Allocate a page-aligned, populated, locked mapping for a published
    /// region. The mapping is zero-filled by the kernel and remains stable
    /// until this owner is dropped before publication, or for process lifetime
    /// once activated for the current emulator API.
    pub fn new(name: &'static str, len: usize) -> Result<Self, RegionError> {
        if len == 0 {
            return Err(RegionError::Empty { name });
        }
        if !len.is_multiple_of(PAGE_SIZE) {
            return Err(RegionError::NotPageMultiple { name, len });
        }

        let ptr = map_region(name, len)?;
        Ok(Self {
            name,
            ptr,
            len,
            published: false,
        })
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_published(&self) -> bool {
        self.published
    }

    pub fn gva(&self) -> u64 {
        self.ptr.as_ptr() as usize as u64
    }

    pub fn is_page_aligned(&self) -> bool {
        (self.gva() as usize).is_multiple_of(PAGE_SIZE)
    }

    pub fn as_slice(&self) -> Result<&[u8], RegionError> {
        self.ensure_unpublished()?;
        // Safety: `ptr` came from an mmap with `len` bytes and remains live
        // until `Drop`; publication has not handed out static aliases.
        Ok(unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) })
    }

    pub fn as_mut_slice(&mut self) -> Result<&mut [u8], RegionError> {
        self.ensure_unpublished()?;
        // Safety: caller has `&mut self`, and the region is unpublished.
        Ok(unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) })
    }

    fn ensure_unpublished(&self) -> Result<(), RegionError> {
        if self.published {
            return Err(RegionError::Published { name: self.name });
        }
        Ok(())
    }

    fn mark_published(&mut self) {
        self.published = true;
    }

    unsafe fn static_array<const N: usize>(&mut self) -> Result<&'static mut [u8; N], RegionError> {
        self.ensure_unpublished()?;
        if self.len != N {
            return Err(RegionError::WrongSize {
                name: self.name,
                expected: N,
                actual: self.len,
            });
        }
        self.mark_published();
        // Safety: upheld by the caller of `HarnessRegions::activate_for_emu`.
        Ok(unsafe { &mut *(self.ptr.as_ptr() as *mut [u8; N]) })
    }

    unsafe fn static_prefix(&mut self, len: usize) -> Result<&'static mut [u8], RegionError> {
        self.ensure_unpublished()?;
        if len > self.len {
            return Err(RegionError::WrongSize {
                name: self.name,
                expected: len,
                actual: self.len,
            });
        }
        self.mark_published();
        // Safety: upheld by the caller of `HarnessRegions::activate_for_emu`;
        // `len <= self.len` was checked above.
        Ok(unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), len) })
    }
}

impl Drop for PublishedRegion {
    fn drop(&mut self) {
        // Safety: `ptr` was returned by `mmap` for exactly `len` bytes and this
        // owner is responsible for unmapping unless it is intentionally kept
        // alive by `ActiveEmuRegions`.
        unsafe {
            libc::munmap(self.ptr.as_ptr().cast(), self.len);
        }
    }
}

#[cfg(target_os = "linux")]
fn map_region(name: &'static str, len: usize) -> Result<NonNull<u8>, RegionError> {
    // Safety: arguments request a private anonymous read/write mapping. The
    // returned pointer is checked against MAP_FAILED before use.
    let raw = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_LOCKED | libc::MAP_POPULATE,
            -1,
            0,
        )
    };
    if raw == libc::MAP_FAILED {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        return Err(RegionError::Map { name, len, errno });
    }
    Ok(NonNull::new(raw.cast::<u8>()).expect("mmap returned null without MAP_FAILED"))
}

#[cfg(not(target_os = "linux"))]
fn map_region(name: &'static str, len: usize) -> Result<NonNull<u8>, RegionError> {
    Err(RegionError::Map {
        name,
        len,
        errno: 0,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionDescriptor {
    pub name: &'static str,
    pub gva: u64,
    pub len: u64,
    pub writable: bool,
}

pub struct SramRegion {
    backing: PublishedRegion,
    logical_len: usize,
}

impl SramRegion {
    pub fn new(logical_len: usize) -> Result<Self, RegionError> {
        validate_sram_len(logical_len)?;
        let mapped_len = logical_len.next_multiple_of(PAGE_SIZE);
        Ok(Self {
            backing: PublishedRegion::new("sram", mapped_len)?,
            logical_len,
        })
    }

    pub fn logical_len(&self) -> usize {
        self.logical_len
    }

    pub fn mapped_len(&self) -> usize {
        self.backing.len()
    }

    fn descriptor(&self) -> RegionDescriptor {
        descriptor_for(&self.backing)
    }

    unsafe fn emu_slice(&mut self) -> Result<&'static mut [u8], RegionError> {
        // Safety: upheld by the caller of `HarnessRegions::activate_for_emu`.
        unsafe { self.backing.static_prefix(self.logical_len) }
    }
}

pub struct HarnessRegions {
    wram: PublishedRegion,
    framebuffer: PublishedRegion,
    meta: PublishedRegion,
    vram: Option<PublishedRegion>,
    sram: Option<SramRegion>,
}

pub struct ActiveEmuRegions {
    _regions: ManuallyDrop<HarnessRegions>,
    pub buffers: RegionBuffers,
}

impl HarnessRegions {
    pub fn required() -> Result<Self, RegionError> {
        Ok(Self {
            wram: PublishedRegion::new("wram", WRAM_SIZE)?,
            framebuffer: PublishedRegion::new("framebuffer", FB_BYTES)?,
            meta: PublishedRegion::new("meta", META_SIZE)?,
            vram: None,
            sram: None,
        })
    }

    pub fn with_optional(vram: bool, sram_len: Option<usize>) -> Result<Self, RegionError> {
        let mut regions = Self::required()?;
        regions.vram = if vram {
            Some(PublishedRegion::new("vram", VRAM_SIZE)?)
        } else {
            None
        };
        regions.sram = match sram_len {
            Some(len) => Some(SramRegion::new(len)?),
            None => None,
        };
        Ok(regions)
    }

    pub fn wram(&self) -> &PublishedRegion {
        &self.wram
    }

    pub fn wram_mut(&mut self) -> &mut PublishedRegion {
        &mut self.wram
    }

    pub fn framebuffer(&self) -> &PublishedRegion {
        &self.framebuffer
    }

    pub fn meta(&self) -> &PublishedRegion {
        &self.meta
    }

    pub fn vram(&self) -> Option<&PublishedRegion> {
        self.vram.as_ref()
    }

    pub fn sram(&self) -> Option<&SramRegion> {
        self.sram.as_ref()
    }

    pub fn descriptors(&self) -> Vec<RegionDescriptor> {
        let mut out = Vec::with_capacity(5);
        out.push(descriptor_for(&self.wram));
        out.push(descriptor_for(&self.framebuffer));
        out.push(descriptor_for(&self.meta));
        if let Some(region) = &self.vram {
            out.push(descriptor_for(region));
        }
        if let Some(region) = &self.sram {
            out.push(region.descriptor());
        }
        out
    }

    /// Consume the region owner and bridge it into the current emulator API.
    ///
    /// # Safety
    ///
    /// `refwork_emu::RegionBuffers` currently requires static published
    /// slices. This method therefore consumes the owner and returns an active
    /// guard that keeps mappings alive for process lifetime. The returned guard
    /// must live at least as long as the `Core` built from `buffers`, and code
    /// must not manufacture additional mutable aliases to the same mappings.
    pub unsafe fn activate_for_emu(mut self) -> Result<ActiveEmuRegions, RegionError> {
        self.validate_emu_sizes()?;

        let buffers = RegionBuffers {
            // Safety: this method consumes the owner and stores it inside
            // `ActiveEmuRegions`, preventing safe post-publication access.
            wram: unsafe { self.wram.static_array::<WRAM_SIZE>()? },
            vram: match &mut self.vram {
                Some(region) => {
                    // Safety: same contract as above.
                    Some(unsafe { region.static_array::<VRAM_SIZE>()? })
                }
                None => None,
            },
            sram: match &mut self.sram {
                Some(region) => {
                    // Safety: same contract as above.
                    Some(unsafe { region.emu_slice()? })
                }
                None => None,
            },
        };

        Ok(ActiveEmuRegions {
            _regions: ManuallyDrop::new(self),
            buffers,
        })
    }

    fn validate_emu_sizes(&self) -> Result<(), RegionError> {
        expect_len(&self.wram, WRAM_SIZE)?;
        if let Some(vram) = &self.vram {
            expect_len(vram, VRAM_SIZE)?;
        }
        Ok(())
    }
}

fn descriptor_for(region: &PublishedRegion) -> RegionDescriptor {
    RegionDescriptor {
        name: region.name(),
        gva: region.gva(),
        len: region.len() as u64,
        writable: false,
    }
}

fn expect_len(region: &PublishedRegion, expected: usize) -> Result<(), RegionError> {
    if region.len() == expected {
        Ok(())
    } else {
        Err(RegionError::WrongSize {
            name: region.name(),
            expected,
            actual: region.len(),
        })
    }
}

fn validate_sram_len(len: usize) -> Result<(), RegionError> {
    if len.is_power_of_two() && (2048..=512 * 1024).contains(&len) {
        Ok(())
    } else {
        Err(RegionError::InvalidSramLen { len })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_lengths() {
        assert!(matches!(
            PublishedRegion::new("zero", 0),
            Err(RegionError::Empty { name: "zero" })
        ));
        assert!(matches!(
            PublishedRegion::new("bad", PAGE_SIZE + 1),
            Err(RegionError::NotPageMultiple {
                name: "bad",
                len
            }) if len == PAGE_SIZE + 1
        ));
    }

    #[test]
    fn region_is_page_aligned_zeroed_and_mutable_before_publication() {
        let mut region = PublishedRegion::new("meta", META_SIZE).unwrap();
        assert_eq!(region.len(), META_SIZE);
        assert!(region.is_page_aligned());
        assert!(region.as_slice().unwrap().iter().all(|&b| b == 0));
        region.as_mut_slice().unwrap()[0] = 7;
        assert_eq!(region.as_slice().unwrap()[0], 7);
    }

    #[test]
    fn required_regions_have_stable_descriptors() {
        let regions = HarnessRegions::required().unwrap();
        let desc = regions.descriptors();
        assert_eq!(desc.len(), 3);
        assert_eq!(desc[0].name, "wram");
        assert_eq!(desc[0].len, WRAM_SIZE as u64);
        assert_eq!(desc[1].name, "framebuffer");
        assert_eq!(desc[1].len, FB_BYTES as u64);
        assert_eq!(desc[2].name, "meta");
        assert_eq!(desc[2].len, META_SIZE as u64);
        assert!(desc
            .iter()
            .all(|d| (d.gva as usize).is_multiple_of(PAGE_SIZE)));
        assert!(desc.iter().all(|d| !d.writable));
    }

    #[test]
    fn optional_regions_are_explicit() {
        let regions = HarnessRegions::with_optional(true, Some(8192)).unwrap();
        let desc = regions.descriptors();
        let names: Vec<_> = desc.iter().map(|d| d.name).collect();
        assert_eq!(names, ["wram", "framebuffer", "meta", "vram", "sram"]);
        assert_eq!(regions.sram().unwrap().logical_len(), 8192);
        assert_eq!(regions.sram().unwrap().mapped_len(), 8192);
    }

    #[test]
    fn sram_logical_len_can_be_smaller_than_mapping_len() {
        let regions = HarnessRegions::with_optional(false, Some(2048)).unwrap();
        let sram = regions.sram().unwrap();
        assert_eq!(sram.logical_len(), 2048);
        assert_eq!(sram.mapped_len(), PAGE_SIZE);
        let sram_desc = regions
            .descriptors()
            .into_iter()
            .find(|d| d.name == "sram")
            .unwrap();
        assert_eq!(sram_desc.len, PAGE_SIZE as u64);
    }

    #[test]
    fn rejects_invalid_sram_lengths() {
        assert!(matches!(
            HarnessRegions::with_optional(false, Some(1024)),
            Err(RegionError::InvalidSramLen { len: 1024 })
        ));
        assert!(matches!(
            HarnessRegions::with_optional(false, Some(3072)),
            Err(RegionError::InvalidSramLen { len: 3072 })
        ));
    }

    #[test]
    fn activation_marks_regions_published_and_preserves_logical_sram_len() {
        let regions = HarnessRegions::with_optional(true, Some(2048)).unwrap();
        // Safety: this test only inspects the returned active guard and does
        // not construct additional aliases.
        let active = unsafe { regions.activate_for_emu() }.unwrap();
        assert_eq!(active.buffers.wram.len(), WRAM_SIZE);
        assert_eq!(active.buffers.vram.as_ref().unwrap().len(), VRAM_SIZE);
        assert_eq!(active.buffers.sram.as_ref().unwrap().len(), 2048);
    }

    #[test]
    fn published_region_blocks_owner_slices_and_repeated_publication() {
        let mut region = PublishedRegion::new("wram", WRAM_SIZE).unwrap();
        {
            // Safety: this test drops the returned reference before checking
            // the owner rejects further access.
            let wram = unsafe { region.static_array::<WRAM_SIZE>() }.unwrap();
            assert_eq!(wram.len(), WRAM_SIZE);
        }

        assert!(matches!(
            region.as_slice(),
            Err(RegionError::Published { name: "wram" })
        ));
        assert!(matches!(
            region.as_mut_slice(),
            Err(RegionError::Published { name: "wram" })
        ));
        assert!(matches!(
            unsafe { region.static_array::<WRAM_SIZE>() },
            Err(RegionError::Published { name: "wram" })
        ));
    }
}
