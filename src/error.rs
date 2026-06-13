//! Type d'erreur central de la librairie. Toute fonction faillible renvoie
//! `Result<_, KontractError>` plutôt que de paniquer (cf. CLAUDE.md).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum KontractError {
    #[error("sous-jacent inconnu : {0}")]
    UnknownAsset(String),

    #[error("date d'observation hors de la timeline : {0}")]
    TimeOutOfRange(f64),

    #[error("pas de temps hors de la trajectoire : index {0}")]
    StepOutOfRange(usize),

    #[error("trajectoire incohérente : {0}")]
    InconsistentPath(String),

    #[error("erreur de (dé)sérialisation : {0}")]
    Serde(String),

    #[error("contrat mal formé : {0}")]
    MalformedContract(String),
}
