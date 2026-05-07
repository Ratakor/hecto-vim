#[derive(Copy, Clone, Debug)]
pub enum GraphemeWidth {
    Half,
    Full,
    Custom(usize),
}
impl From<GraphemeWidth> for usize {
    fn from(val: GraphemeWidth) -> Self {
        match val {
            GraphemeWidth::Half => 1,
            GraphemeWidth::Full => 2,
            GraphemeWidth::Custom(width) => width,
        }
    }
}
