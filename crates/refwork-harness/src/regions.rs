use std::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};
use std::fmt;
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
    Layout {
        name: &'static str,
        len: usize,
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
            RegionError::Layout { name, len } => {
                write!(
                    f,
                    "region `{name}` length {len} cannot form an allocation layout"
                )
            }
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
    layout: Layout,
    published: bool,
}

impl PublishedRegion {
    pub fn new(name: &'static str, len: usize) -> Result<Self, RegionError> {
        if len == 0 {
            return Err(RegionError::Empty { name });
        }
        if !len.is_multiple_of(PAGE_SIZE) {
            return Err(RegionError::NotPageMultiple { name, len });
        }
        let layout = Layout::from_size_align(len, PAGE_SIZE)
            .map_err(|_| RegionError::Layout { name, len })?;
        // Safety: `layout` is non-zero-sized and uses a power-of-two alignment.
        let raw = unsafe { alloc_zeroed(layout) };
        let ptr = NonNull::new(raw).unwrap_or_else(|| handle_alloc_error(layout));
        Ok(Self {
            name,
            ptr,
            len,
            layout,
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

    pub fn as_slice(&self) -> &[u8] {
        // Safety: `ptr` came from an allocation with `len` bytes and remains
        // live until `Drop`.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    pub fn as_mut_slice(&mut self) -> Result<&mut [u8], RegionError> {
        if self.published {
            return Err(RegionError::Published { name: self.name });
        }
        // Safety: caller has `&mut self`, so no other mutable slice can be
        // produced through this owner at the same time.
        Ok(unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) })
    }

    fn mark_published(&mut self) {
        self.published = true;
    }

    unsafe fn static_array<const N: usize>(&mut self) -> Result<&'static mut [u8; N], RegionError> {
        if self.len != N {
            return Err(RegionError::WrongSize {
                name: self.name,
                expected: N,
                actual: self.len,
            });
        }
        self.mark_published();
        // Safety: upheld by the caller of `HarnessRegions::emu_buffers`.
        Ok(unsafe { &mut *(self.ptr.as_ptr() as *mut [u8; N]) })
    }

    unsafe fn static_slice(&mut self) -> &'static mut [u8] {
        self.mark_published();
        // Safety: upheld by the caller of `HarnessRegions::emu_buffers`.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for PublishedRegion {
    fn drop(&mut self) {
        // Safety: `ptr` was allocated with this exact `layout` in `new`, and
        // `PublishedRegion` is the sole owner responsible for deallocation.
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionDescriptor {
    pub name: &'static str,
    pub gva: u64,
    pub len: u64,
    pub writable: bool,
}

pub struct HarnessRegions {
    pub wram: PublishedRegion,
    pub framebuffer: PublishedRegion,
    pub meta: PublishedRegion,
    pub vram: Option<PublishedRegion>,
    pub sram: Option<PublishedRegion>,
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
            Some(len) => Some(PublishedRegion::new("sram", len)?),
            None => None,
        };
        Ok(regions)
    }

    pub fn descriptors(&self) -> Vec<RegionDescriptor> {
        let mut out = Vec::with_capacity(5);
        push_descriptor(&mut out, &self.wram);
        push_descriptor(&mut out, &self.framebuffer);
        push_descriptor(&mut out, &self.meta);
        if let Some(region) = &self.vram {
            push_descriptor(&mut out, region);
        }
        if let Some(region) = &self.sram {
            push_descriptor(&mut out, region);
        }
        out
    }

    /// Bridge the owned region set into the current emulator core API.
    ///
    /// # Safety
    ///
    /// The returned references are widened to `'static` because
    /// `refwork_emu::RegionBuffers` currently requires static published
    /// slices. The caller must keep this `HarnessRegions` value alive and must
    /// not access or drop it until the `Core` built from the returned buffers
    /// has stopped. This method marks the bridged regions as published so this
    /// owner will no longer hand out mutable slices through safe APIs.
    pub unsafe fn emu_buffers(&mut self) -> Result<RegionBuffers, RegionError> {
        Ok(RegionBuffers {
            // Safety: this method's contract requires the owner to outlive the
            // emulator core and forbids aliasing access.
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
                    Some(unsafe { region.static_slice() })
                }
                None => None,
            },
        })
    }
}

fn push_descriptor(out: &mut Vec<RegionDescriptor>, region: &PublishedRegion) {
    out.push(RegionDescriptor {
        name: region.name(),
        gva: region.gva(),
        len: region.len() as u64,
        writable: false,
    });
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
        assert!(region.as_slice().iter().all(|&b| b == 0));
        region.as_mut_slice().unwrap()[0] = 7;
        assert_eq!(region.as_slice()[0], 7);
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
        let names: Vec<_> = regions.descriptors().iter().map(|d| d.name).collect();
        assert_eq!(names, ["wram", "framebuffer", "meta", "vram", "sram"]);
    }

    #[test]
    fn emu_bridge_marks_regions_published_and_blocks_mutation() {
        let mut regions = HarnessRegions::with_optional(true, Some(8192)).unwrap();
        {
            // Safety: this test immediately drops the returned buffer
            // references before accessing the owner again.
            let buffers = unsafe { regions.emu_buffers() }.unwrap();
            assert_eq!(buffers.wram.len(), WRAM_SIZE);
            assert_eq!(buffers.vram.as_ref().unwrap().len(), VRAM_SIZE);
            assert_eq!(buffers.sram.as_ref().unwrap().len(), 8192);
        }

        assert!(regions.wram.is_published());
        assert!(regions.vram.as_ref().unwrap().is_published());
        assert!(regions.sram.as_ref().unwrap().is_published());
        assert!(matches!(
            regions.wram.as_mut_slice(),
            Err(RegionError::Published { name: "wram" })
        ));
    }

    #[test]
    fn emu_bridge_rejects_wrong_sized_wram() {
        let mut regions = HarnessRegions {
            wram: PublishedRegion::new("wram", PAGE_SIZE).unwrap(),
            framebuffer: PublishedRegion::new("framebuffer", FB_BYTES).unwrap(),
            meta: PublishedRegion::new("meta", META_SIZE).unwrap(),
            vram: None,
            sram: None,
        };
        let err = match unsafe { regions.emu_buffers() } {
            Ok(_) => panic!("wrong-sized wram should fail"),
            Err(err) => err,
        };
        assert_eq!(
            err,
            RegionError::WrongSize {
                name: "wram",
                expected: WRAM_SIZE,
                actual: PAGE_SIZE
            }
        );
    }
}
