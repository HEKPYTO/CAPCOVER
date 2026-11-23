#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifyReport {
    pub n: usize,
    pub uncovered: usize,
    pub first_uncovered: Option<usize>,
    pub full: bool,
}
