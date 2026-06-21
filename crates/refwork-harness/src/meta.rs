#![forbid(unsafe_code)]

use std::sync::atomic::{compiler_fence, Ordering};

use refwork_protocol::FaultCode;

pub const META_SIZE: usize = 4096;
pub const META_VERSION: u32 = 1;
pub const EMU_VERSION_LEN: usize = 24;

const VERSION_OFF: usize = 0x00;
const STATUS_OFF: usize = 0x04;
const FRAME_OFF: usize = 0x08;
const LAST_PAD_OFF: usize = 0x10;
const RESERVED_OFF: usize = 0x12;
const FAULT_CODE_OFF: usize = 0x14;
const CART_HASH_OFF: usize = 0x18;
const EMU_VERSION_OFF: usize = 0x38;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaStatus {
    Init = 0,
    Ready = 1,
    Running = 2,
    Faulted = 3,
}

pub struct MetaPage<'a> {
    bytes: &'a mut [u8; META_SIZE],
}

impl<'a> MetaPage<'a> {
    pub fn new(bytes: &'a mut [u8; META_SIZE]) -> Self {
        bytes.fill(0);
        let mut page = Self { bytes };
        page.write_u32(VERSION_OFF, META_VERSION);
        page.set_status(MetaStatus::Init);
        page
    }

    pub fn bytes(&self) -> &[u8; META_SIZE] {
        self.bytes
    }

    pub fn bytes_mut(&mut self) -> &mut [u8; META_SIZE] {
        self.bytes
    }

    pub fn set_status(&mut self, status: MetaStatus) {
        self.write_u32(STATUS_OFF, status as u32);
    }

    pub fn set_ready(&mut self) {
        self.set_frame(0);
        self.set_last_pad(0);
        self.write_u32(FAULT_CODE_OFF, 0);
        self.publish_status(MetaStatus::Ready);
    }

    pub fn set_running_frame(&mut self, frame: u64, last_pad: u16) {
        self.set_frame(frame);
        self.set_last_pad(last_pad);
        self.publish_status(MetaStatus::Running);
    }

    pub fn set_fault(&mut self, frame: u64, code: FaultCode) {
        self.set_frame(frame);
        self.write_u32(FAULT_CODE_OFF, fault_code_value(code));
        self.publish_status(MetaStatus::Faulted);
    }

    pub fn set_frame(&mut self, frame: u64) {
        self.write_u64(FRAME_OFF, frame);
    }

    pub fn set_last_pad(&mut self, pad: u16) {
        self.write_u16(LAST_PAD_OFF, pad & 0x0fff);
        self.write_u16(RESERVED_OFF, 0);
    }

    pub fn set_cart_hash(&mut self, hash: [u8; 32]) {
        self.bytes[CART_HASH_OFF..CART_HASH_OFF + 32].copy_from_slice(&hash);
    }

    pub fn set_emu_version(&mut self, version: &str) {
        let dst = &mut self.bytes[EMU_VERSION_OFF..EMU_VERSION_OFF + EMU_VERSION_LEN];
        dst.fill(0);
        let src = version.as_bytes();
        let n = src.len().min(EMU_VERSION_LEN);
        dst[..n].copy_from_slice(&src[..n]);
    }

    fn write_u16(&mut self, off: usize, value: u16) {
        self.bytes[off..off + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(&mut self, off: usize, value: u32) {
        self.bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, off: usize, value: u64) {
        self.bytes[off..off + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn publish_status(&mut self, status: MetaStatus) {
        compiler_fence(Ordering::Release);
        self.set_status(status);
    }
}

pub const fn fault_code_value(code: FaultCode) -> u32 {
    match code {
        FaultCode::BadProto => 0,
        FaultCode::BadGame => 1,
        FaultCode::RegionRegFailed => 2,
        FaultCode::EmuHalt => 3,
        FaultCode::ProtocolOrder => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16_at(bytes: &[u8], off: usize) -> u16 {
        u16::from_le_bytes(bytes[off..off + 2].try_into().unwrap())
    }

    fn u32_at(bytes: &[u8], off: usize) -> u32 {
        u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap())
    }

    fn u64_at(bytes: &[u8], off: usize) -> u64 {
        u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap())
    }

    #[test]
    fn new_page_sets_version_and_init_status() {
        let mut bytes = [0xffu8; META_SIZE];
        let page = MetaPage::new(&mut bytes);
        assert_eq!(u32_at(page.bytes(), VERSION_OFF), META_VERSION);
        assert_eq!(u32_at(page.bytes(), STATUS_OFF), MetaStatus::Init as u32);
        assert_eq!(u64_at(page.bytes(), FRAME_OFF), 0);
        assert_eq!(u16_at(page.bytes(), LAST_PAD_OFF), 0);
        assert_eq!(u16_at(page.bytes(), RESERVED_OFF), 0);
    }

    #[test]
    fn ready_and_running_write_exact_offsets() {
        let mut bytes = [0u8; META_SIZE];
        let mut page = MetaPage::new(&mut bytes);
        page.set_ready();
        assert_eq!(u32_at(page.bytes(), STATUS_OFF), MetaStatus::Ready as u32);
        assert_eq!(u64_at(page.bytes(), FRAME_OFF), 0);

        page.set_running_frame(42, 0xf123);
        assert_eq!(u32_at(page.bytes(), STATUS_OFF), MetaStatus::Running as u32);
        assert_eq!(u64_at(page.bytes(), FRAME_OFF), 42);
        assert_eq!(u16_at(page.bytes(), LAST_PAD_OFF), 0x0123);
        assert_eq!(u16_at(page.bytes(), RESERVED_OFF), 0);
    }

    #[test]
    fn fault_writes_status_frame_and_code() {
        let mut bytes = [0u8; META_SIZE];
        let mut page = MetaPage::new(&mut bytes);
        page.set_fault(7, FaultCode::ProtocolOrder);
        assert_eq!(u32_at(page.bytes(), STATUS_OFF), MetaStatus::Faulted as u32);
        assert_eq!(u64_at(page.bytes(), FRAME_OFF), 7);
        assert_eq!(u32_at(page.bytes(), FAULT_CODE_OFF), 4);
    }

    #[test]
    fn cart_hash_and_version_are_padded_at_spec_offsets() {
        let mut bytes = [0u8; META_SIZE];
        let mut page = MetaPage::new(&mut bytes);
        let hash = [0xabu8; 32];
        page.set_cart_hash(hash);
        page.set_emu_version("emu-0.1");

        assert_eq!(&page.bytes()[CART_HASH_OFF..CART_HASH_OFF + 32], &hash);
        let version = &page.bytes()[EMU_VERSION_OFF..EMU_VERSION_OFF + EMU_VERSION_LEN];
        assert_eq!(&version[..7], b"emu-0.1");
        assert!(version[7..].iter().all(|&b| b == 0));
    }

    #[test]
    fn long_version_is_truncated_without_overflow() {
        let mut bytes = [0u8; META_SIZE];
        let mut page = MetaPage::new(&mut bytes);
        page.set_emu_version("123456789012345678901234567890");
        assert_eq!(
            &page.bytes()[EMU_VERSION_OFF..EMU_VERSION_OFF + EMU_VERSION_LEN],
            b"123456789012345678901234"
        );
    }
}
