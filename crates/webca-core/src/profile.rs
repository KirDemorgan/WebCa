use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

use crate::{ControlId, ControlSetting};

const MAX_NAME_CHARS: usize = 32;
const DEFAULT_PROFILE: &str = "Default";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    #[error("profile name cannot be empty")]
    EmptyName,
    #[error("profile name cannot be longer than {MAX_NAME_CHARS} characters")]
    NameTooLong,
    #[error("a profile named `{0}` already exists")]
    DuplicateName(String),
    #[error("no profile named `{0}`")]
    NotFound(String),
    #[error("the last profile cannot be removed")]
    LastProfile,
    #[error("profile file version {0} is not supported")]
    UnsupportedVersion(u32),
}

/// A named set of control values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawProfile")]
pub struct Profile {
    name: String,
    #[serde(default)]
    settings: BTreeMap<ControlId, ControlSetting>,
}

#[derive(Deserialize)]
struct RawProfile {
    name: String,
    #[serde(default)]
    settings: BTreeMap<ControlId, ControlSetting>,
}

impl TryFrom<RawProfile> for Profile {
    type Error = ProfileError;

    fn try_from(raw: RawProfile) -> Result<Self, Self::Error> {
        Ok(Self { name: validate_name(&raw.name)?, settings: raw.settings })
    }
}

impl Profile {
    pub fn new(name: &str) -> Result<Self, ProfileError> {
        Ok(Self { name: validate_name(name)?, settings: BTreeMap::new() })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn get(&self, id: ControlId) -> Option<ControlSetting> {
        self.settings.get(&id).copied()
    }

    pub fn set(&mut self, id: ControlId, setting: ControlSetting) {
        self.settings.insert(id, setting);
    }

    pub fn remove(&mut self, id: ControlId) -> Option<ControlSetting> {
        self.settings.remove(&id)
    }

    pub fn settings(&self) -> impl Iterator<Item = (ControlId, ControlSetting)> + '_ {
        self.settings.iter().map(|(id, setting)| (*id, *setting))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }
}

fn validate_name(name: &str) -> Result<String, ProfileError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ProfileError::EmptyName);
    }
    if trimmed.chars().count() > MAX_NAME_CHARS {
        return Err(ProfileError::NameTooLong);
    }
    Ok(trimmed.to_owned())
}

fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// Identifies a physical camera across sessions by its device interface path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub struct CameraKey(String);

impl CameraKey {
    #[must_use]
    pub fn new(device_path: &str) -> Self {
        Self(device_path.trim().to_lowercase())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for CameraKey {
    fn from(value: String) -> Self {
        Self::new(&value)
    }
}

impl From<CameraKey> for String {
    fn from(key: CameraKey) -> Self {
        key.0
    }
}

impl fmt::Display for CameraKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// All profiles of one camera and which of them is active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawCameraProfiles")]
pub struct CameraProfiles {
    active: String,
    profiles: Vec<Profile>,
}

#[derive(Deserialize)]
struct RawCameraProfiles {
    active: String,
    profiles: Vec<Profile>,
}

impl TryFrom<RawCameraProfiles> for CameraProfiles {
    type Error = ProfileError;

    fn try_from(raw: RawCameraProfiles) -> Result<Self, Self::Error> {
        let mut camera = Self { active: String::new(), profiles: Vec::with_capacity(raw.profiles.len()) };
        for profile in raw.profiles {
            if camera.position(profile.name()).is_some() {
                return Err(ProfileError::DuplicateName(profile.name));
            }
            camera.profiles.push(profile);
        }
        let first = camera.profiles.first().ok_or(ProfileError::LastProfile)?.name.clone();
        camera.active =
            camera.position(&raw.active).map_or(first, |index| camera.profiles[index].name.clone());
        Ok(camera)
    }
}

impl Default for CameraProfiles {
    fn default() -> Self {
        Self {
            active: DEFAULT_PROFILE.to_owned(),
            profiles: vec![Profile::new(DEFAULT_PROFILE).expect("default profile name is valid")],
        }
    }
}

impl CameraProfiles {
    #[must_use]
    pub fn active(&self) -> &Profile {
        &self.profiles[self.active_index()]
    }

    pub fn active_mut(&mut self) -> &mut Profile {
        let index = self.active_index();
        &mut self.profiles[index]
    }

    pub fn profiles(&self) -> impl Iterator<Item = &Profile> {
        self.profiles.iter()
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.position(name).map(|index| &self.profiles[index])
    }

    pub fn select(&mut self, name: &str) -> Result<&Profile, ProfileError> {
        let index = self.position(name).ok_or_else(|| ProfileError::NotFound(name.to_owned()))?;
        self.active.clone_from(&self.profiles[index].name);
        Ok(&self.profiles[index])
    }

    /// Adds a profile that starts as a copy of the active one.
    pub fn add(&mut self, name: &str) -> Result<&Profile, ProfileError> {
        let mut profile = self.active().clone();
        profile.name = validate_name(name)?;
        if self.position(&profile.name).is_some() {
            return Err(ProfileError::DuplicateName(profile.name));
        }
        self.profiles.push(profile);
        Ok(self.profiles.last().expect("a profile was just pushed"))
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), ProfileError> {
        let index = self.position(from).ok_or_else(|| ProfileError::NotFound(from.to_owned()))?;
        let new_name = validate_name(to)?;
        if self.position(&new_name).is_some_and(|other| other != index) {
            return Err(ProfileError::DuplicateName(new_name));
        }
        let was_active = index == self.active_index();
        self.profiles[index].name.clone_from(&new_name);
        if was_active {
            self.active = new_name;
        }
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<Profile, ProfileError> {
        let index = self.position(name).ok_or_else(|| ProfileError::NotFound(name.to_owned()))?;
        if self.profiles.len() == 1 {
            return Err(ProfileError::LastProfile);
        }
        let was_active = index == self.active_index();
        let removed = self.profiles.remove(index);
        if was_active {
            self.active.clone_from(&self.profiles[0].name);
        }
        Ok(removed)
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.profiles.iter().position(|profile| same_name(&profile.name, name))
    }

    fn active_index(&self) -> usize {
        self.position(&self.active).expect("active profile always exists")
    }
}

/// Everything WebCa persists about the user's cameras.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawProfileBook")]
pub struct ProfileBook {
    version: u32,
    #[serde(default)]
    cameras: BTreeMap<CameraKey, CameraProfiles>,
}

#[derive(Deserialize)]
struct RawProfileBook {
    version: u32,
    #[serde(default)]
    cameras: BTreeMap<CameraKey, CameraProfiles>,
}

impl TryFrom<RawProfileBook> for ProfileBook {
    type Error = ProfileError;

    fn try_from(raw: RawProfileBook) -> Result<Self, Self::Error> {
        if raw.version > ProfileBook::VERSION {
            return Err(ProfileError::UnsupportedVersion(raw.version));
        }
        Ok(Self { version: ProfileBook::VERSION, cameras: raw.cameras })
    }
}

impl Default for ProfileBook {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileBook {
    pub const VERSION: u32 = 1;

    #[must_use]
    pub fn new() -> Self {
        Self { version: Self::VERSION, cameras: BTreeMap::new() }
    }

    #[must_use]
    pub fn camera(&self, key: &CameraKey) -> Option<&CameraProfiles> {
        self.cameras.get(key)
    }

    /// Profiles of `key`, created with a single default profile on first use.
    pub fn camera_mut(&mut self, key: &CameraKey) -> &mut CameraProfiles {
        self.cameras.entry(key.clone()).or_default()
    }

    pub fn set_camera(&mut self, key: &CameraKey, profiles: CameraProfiles) {
        self.cameras.insert(key.clone(), profiles);
    }

    pub fn cameras(&self) -> impl Iterator<Item = (&CameraKey, &CameraProfiles)> {
        self.cameras.iter()
    }

    #[must_use]
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("profile book always serializes")
    }

    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera_with(names: &[&str]) -> CameraProfiles {
        let mut camera = CameraProfiles::default();
        for name in names {
            camera.add(name).unwrap();
        }
        camera
    }

    #[test]
    fn profile_names_are_trimmed_and_validated() {
        assert_eq!(Profile::new("  Evening ").unwrap().name(), "Evening");
        assert_eq!(Profile::new("   "), Err(ProfileError::EmptyName));
        assert_eq!(Profile::new(&"x".repeat(33)), Err(ProfileError::NameTooLong));
        assert!(Profile::new(&"я".repeat(32)).is_ok());
    }

    #[test]
    fn profile_stores_settings() {
        let mut profile = Profile::new("Day").unwrap();
        assert!(profile.is_empty());
        profile.set(ControlId::Contrast, ControlSetting::manual(140));
        profile.set(ControlId::Brightness, ControlSetting::manual(120));
        assert_eq!(profile.get(ControlId::Contrast), Some(ControlSetting::manual(140)));
        assert_eq!(profile.get(ControlId::Hue), None);
        let ids: Vec<_> = profile.settings().map(|(id, _)| id).collect();
        assert_eq!(ids, [ControlId::Brightness, ControlId::Contrast]);
        assert_eq!(profile.remove(ControlId::Contrast), Some(ControlSetting::manual(140)));
        assert_eq!(profile.remove(ControlId::Contrast), None);
    }

    #[test]
    fn camera_key_is_normalized() {
        let key = CameraKey::new(r" \\?\USB#VID_1D6C ");
        assert_eq!(key.as_str(), r"\\?\usb#vid_1d6c");
        assert_eq!(key.to_string(), key.as_str());
        assert_eq!(CameraKey::from(String::from("ABC")), CameraKey::new("abc"));
        assert_eq!(String::from(CameraKey::new("X")), "x");
    }

    #[test]
    fn default_camera_has_one_active_profile() {
        let camera = CameraProfiles::default();
        assert_eq!(camera.active().name(), "Default");
        assert_eq!(camera.profiles().count(), 1);
    }

    #[test]
    fn added_profiles_copy_the_active_one() {
        let mut camera = CameraProfiles::default();
        camera.active_mut().set(ControlId::Gamma, ControlSetting::manual(5));
        let added = camera.add("Stream").unwrap();
        assert_eq!(added.get(ControlId::Gamma), Some(ControlSetting::manual(5)));
        assert_eq!(camera.active().name(), "Default");
    }

    #[test]
    fn add_rejects_duplicates_and_bad_names() {
        let mut camera = camera_with(&["Day"]);
        assert_eq!(camera.add("day "), Err(ProfileError::DuplicateName("day".into())));
        assert_eq!(camera.add(""), Err(ProfileError::EmptyName));
    }

    #[test]
    fn select_switches_active_profile() {
        let mut camera = camera_with(&["Day", "Night"]);
        assert_eq!(camera.select("NIGHT").unwrap().name(), "Night");
        assert_eq!(camera.active().name(), "Night");
        assert_eq!(camera.select("Party"), Err(ProfileError::NotFound("Party".into())));
        assert_eq!(camera.get("day").map(Profile::name), Some("Day"));
        assert!(camera.get("Party").is_none());
    }

    #[test]
    fn rename_keeps_active_selection() {
        let mut camera = camera_with(&["Day"]);
        camera.select("Day").unwrap();
        camera.rename("Day", "Morning").unwrap();
        assert_eq!(camera.active().name(), "Morning");
        camera.rename("Default", "Base").unwrap();
        assert_eq!(camera.active().name(), "Morning");
        camera.rename("morning", "Morning").unwrap();
        assert_eq!(camera.rename("Base", "MORNING"), Err(ProfileError::DuplicateName("MORNING".into())));
        assert_eq!(camera.rename("Ghost", "X"), Err(ProfileError::NotFound("Ghost".into())));
        assert_eq!(camera.rename("Base", " "), Err(ProfileError::EmptyName));
    }

    #[test]
    fn remove_falls_back_to_first_profile() {
        let mut camera = camera_with(&["Day", "Night"]);
        camera.select("Night").unwrap();
        assert_eq!(camera.remove("Night").unwrap().name(), "Night");
        assert_eq!(camera.active().name(), "Default");
        camera.remove("Day").unwrap();
        assert_eq!(camera.active().name(), "Default");
        assert_eq!(camera.remove("Default"), Err(ProfileError::LastProfile));
        assert_eq!(camera.remove("Day"), Err(ProfileError::NotFound("Day".into())));
    }

    #[test]
    fn book_creates_cameras_on_demand() {
        let key = CameraKey::new("cam");
        let mut book = ProfileBook::new();
        assert!(book.camera(&key).is_none());
        book.camera_mut(&key).active_mut().set(ControlId::Zoom, ControlSetting::manual(2));
        assert_eq!(book.camera(&key).unwrap().active().get(ControlId::Zoom), Some(ControlSetting::manual(2)));
        assert_eq!(book.cameras().count(), 1);
        let mut replacement = CameraProfiles::default();
        replacement.add("Night").unwrap();
        book.set_camera(&key, replacement.clone());
        assert_eq!(book.camera(&key), Some(&replacement));
    }

    #[test]
    fn book_round_trips_through_toml() {
        let key = CameraKey::new(r"\\?\usb#vid_1d6c&pid_0103");
        let mut book = ProfileBook::new();
        let camera = book.camera_mut(&key);
        camera.active_mut().set(ControlId::WhiteBalance, ControlSetting::auto(4600));
        camera.add("Night").unwrap();
        camera.select("Night").unwrap();
        let text = book.to_toml();
        assert!(text.contains("version = 1"));
        assert_eq!(ProfileBook::from_toml(&text).unwrap(), book);
        assert_eq!(ProfileBook::from_toml(&ProfileBook::default().to_toml()).unwrap(), ProfileBook::new());
    }

    #[test]
    fn book_rejects_newer_versions() {
        let err = ProfileBook::from_toml("version = 99").unwrap_err();
        assert!(err.to_string().contains("profile file version 99 is not supported"));
    }

    #[test]
    fn invalid_camera_entries_are_rejected_or_repaired() {
        let duplicate = r#"
            version = 1
            [cameras.cam]
            active = "A"
            [[cameras.cam.profiles]]
            name = "A"
            [[cameras.cam.profiles]]
            name = "a"
        "#;
        assert!(ProfileBook::from_toml(duplicate).unwrap_err().to_string().contains("already exists"));

        let empty = "version = 1\n[cameras.cam]\nactive = \"A\"\nprofiles = []\n";
        assert!(ProfileBook::from_toml(empty).unwrap_err().to_string().contains("last profile"));

        let unknown_active = r#"
            version = 1
            [cameras.cam]
            active = "Gone"
            [[cameras.cam.profiles]]
            name = "Kept"
        "#;
        let book = ProfileBook::from_toml(unknown_active).unwrap();
        assert_eq!(book.camera(&CameraKey::new("cam")).unwrap().active().name(), "Kept");

        let bad_name = "version = 1\n[cameras.cam]\nactive = \"\"\n[[cameras.cam.profiles]]\nname = \"\"\n";
        assert!(ProfileBook::from_toml(bad_name).unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn errors_have_readable_messages() {
        assert_eq!(ProfileError::NameTooLong.to_string(), "profile name cannot be longer than 32 characters");
        assert_eq!(ProfileError::NotFound("X".into()).to_string(), "no profile named `X`");
    }
}
