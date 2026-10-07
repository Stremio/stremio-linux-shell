use webkit::Feature;
use webkit::Settings;

pub trait SettingsExt {
    fn find_feature(&self, identifier: &str) -> Option<Feature>;
}

impl SettingsExt for Settings {
    fn find_feature(&self, identifier: &str) -> Option<Feature> {
        Settings::all_features().and_then(|features| {
            (0..features.length())
                .filter_map(|index| features.get(index))
                .find(|feature| feature.identifier().as_deref() == Some(identifier))
        })
    }
}
