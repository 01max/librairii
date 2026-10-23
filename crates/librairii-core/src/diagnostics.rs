//! Stable diagnostics presentation. Storage causes and personal paths never
//! become user-facing strings or event-log fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Problem {
    Destination,
    Collision,
    Cancelled,
    LogsUnavailable,
    ExportFailed,
    RecoveryRequired,
}
impl Problem {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Destination => "diagnostics.destination_invalid",
            Self::Collision => "diagnostics.destination_exists",
            Self::Cancelled => "diagnostics.cancelled",
            Self::LogsUnavailable => "diagnostics.logs_unavailable",
            Self::ExportFailed => "diagnostics.export_failed",
            Self::RecoveryRequired => "diagnostics.recovery_required",
        }
    }
    pub fn message(self, locale: &str) -> &'static str {
        let french = locale == "fr" || locale.starts_with("fr-");
        match (self, french) {
            (Self::Destination, false) => {
                "Choose an available folder outside your library for diagnostics."
            }
            (Self::Destination, true) => {
                "Choisissez un dossier disponible en dehors de votre bibliothèque pour les diagnostics."
            }
            (Self::Collision, false) => {
                "A file with this name already exists. Choose another folder or try again."
            }
            (Self::Collision, true) => {
                "Un fichier de ce nom existe déjà. Choisissez un autre dossier ou réessayez."
            }
            (Self::Cancelled, false) => "Diagnostics export was cancelled.",
            (Self::Cancelled, true) => "L’export des diagnostics a été annulé.",
            (Self::LogsUnavailable, false) => {
                "The local event logs could not be read safely. Your library data was not included."
            }
            (Self::LogsUnavailable, true) => {
                "Les journaux locaux n’ont pas pu être lus en toute sécurité. Les données de votre bibliothèque n’ont pas été incluses."
            }
            (Self::ExportFailed, false) => {
                "Diagnostics could not be exported. Check the selected folder and available space, then try again."
            }
            (Self::ExportFailed, true) => {
                "Les diagnostics n’ont pas pu être exportés. Vérifiez le dossier sélectionné et l’espace disponible, puis réessayez."
            }
            (Self::RecoveryRequired, false) => {
                "The export outcome could not be confirmed. Keep the selected files and reopen the library through recovery."
            }
            (Self::RecoveryRequired, true) => {
                "Le résultat de l’export n’a pas pu être confirmé. Conservez les fichiers sélectionnés et rouvrez la bibliothèque via la récupération."
            }
        }
    }
}
