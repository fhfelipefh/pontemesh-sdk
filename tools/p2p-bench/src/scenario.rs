#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    OriginOnly,
    P2pSingleSeeder,
    P2pMesh,
    P2pFallback,
}

impl Scenario {
    pub fn all() -> Vec<Self> {
        vec![
            Self::OriginOnly,
            Self::P2pSingleSeeder,
            Self::P2pMesh,
            Self::P2pFallback,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::OriginOnly => "origin-only",
            Self::P2pSingleSeeder => "p2p-single-seeder",
            Self::P2pMesh => "p2p-mesh",
            Self::P2pFallback => "p2p-fallback",
        }
    }

    pub fn is_p2p(self) -> bool {
        !matches!(self, Self::OriginOnly)
    }
}
