/// Split the slice into maximal groups of same key and use given callback on each group.
/// NOTE: waiting for stabilisation of https://github.com/rust-lang/rust/issues/80552
pub fn for_each_group<'a, T, K, F, V>(mut slice: &'a mut [T], key: K, mut function: F)
where
    K: Fn(&T) -> V,
    F: FnMut(&'a mut [T]),
    V: Eq,
{
    while let Some(curr_key) = slice.first().map(&key) {
        let group_size = slice.iter().take_while(|x| key(x) == curr_key).count();
        let (head, tail) = slice.split_at_mut(group_size);
        slice = tail;
        function(head);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_for_each_group() {
        let mut data = [1, 3, 5, 2, 2, 7];

        for_each_group(
            &mut data,
            |x| x % 2,
            |group| {
                let len = group.len();

                for x in group.iter_mut() {
                    *x *= len;
                }
            },
        );

        assert_eq!(data, [3, 9, 15, 4, 4, 7]);
    }
}
