pub fn common_prefix(v1: &[u8], v2: &[u8]) -> usize {
    let mut count: usize = 0;
    while v1[count] == v2[count] {
        count += 1;
    }
    count
}
