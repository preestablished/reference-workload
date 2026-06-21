pub(crate) const MAX_FAULT_DETAIL_BYTES: usize = 512;

pub(crate) fn bounded_fault_detail(detail: &str) -> String {
    let mut end = detail.len().min(MAX_FAULT_DETAIL_BYTES);
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    if end < detail.len() {
        format!("{}...", &detail[..end])
    } else {
        detail.into()
    }
}
