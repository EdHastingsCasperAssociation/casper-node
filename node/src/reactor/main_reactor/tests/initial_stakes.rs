pub(crate) enum InitialStakes {
    FromVec(Vec<u128>),
    Random { count: usize },
    AllEqual { count: usize, stake: u128 },
}
