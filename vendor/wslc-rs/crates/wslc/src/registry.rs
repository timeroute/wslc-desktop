use crate::{Error, Result};

const DEFAULT_REGISTRY: &str = "docker.io";
const DEFAULT_MIRROR_ENV: &str = "WSLC_REGISTRY_MIRROR";
const MIRROR_ENV_PREFIX: &str = "WSLC_REGISTRY_MIRROR_";

pub(crate) fn resolve_image_reference(image: &str) -> Result<String> {
    resolve_image_reference_with(image, |key| std::env::var(key).ok())
}

pub(crate) fn resolve_image_reference_with<F>(image: &str, mut env: F) -> Result<String>
where
    F: FnMut(&str) -> Option<String>,
{
    let parsed = ImageReference::parse(image);
    let registry = parsed.registry.unwrap_or(DEFAULT_REGISTRY);
    let Some(mirror) = mirror_for_registry(registry, &mut env)? else {
        return Ok(image.to_owned());
    };

    Ok(format!(
        "{}/{}",
        mirror.trim_end_matches('/'),
        parsed.repository_with_tag
    ))
}

fn mirror_for_registry<F>(registry: &str, env: &mut F) -> Result<Option<String>>
where
    F: FnMut(&str) -> Option<String>,
{
    let registry_key = registry
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let exact_key = format!("{MIRROR_ENV_PREFIX}{registry_key}");

    let mirror = env(&exact_key).or_else(|| {
        if registry == DEFAULT_REGISTRY {
            env(DEFAULT_MIRROR_ENV)
        } else {
            None
        }
    });

    match mirror.map(|value| value.trim().to_owned()) {
        Some(value) if value.is_empty() => Err(Error::InvalidInput(format!(
            "registry mirror env var cannot be empty for {registry}"
        ))),
        Some(value) if value.contains('\0') => Err(Error::Nul("registry mirror".to_owned())),
        Some(value) if value.contains("://") => Err(Error::InvalidInput(
            "registry mirror must be an image registry host, not a URL".to_owned(),
        )),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

struct ImageReference<'a> {
    registry: Option<&'a str>,
    repository_with_tag: &'a str,
}

impl<'a> ImageReference<'a> {
    fn parse(image: &'a str) -> Self {
        let (first, rest) = image.split_once('/').unwrap_or((image, ""));
        let has_registry = first == "localhost" || first.contains('.') || first.contains(':');
        if has_registry && !rest.is_empty() {
            Self {
                registry: Some(first),
                repository_with_tag: rest,
            }
        } else {
            Self {
                registry: None,
                repository_with_tag: image,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find_map(|(name, value)| (*name == key).then(|| (*value).to_owned()))
        }
    }

    #[test]
    fn leaves_images_unchanged_without_mirror_env() {
        assert_eq!(
            resolve_image_reference_with("docker.io/library/alpine:latest", env(&[])).unwrap(),
            "docker.io/library/alpine:latest"
        );
        assert_eq!(
            resolve_image_reference_with("alpine:latest", env(&[])).unwrap(),
            "alpine:latest"
        );
    }

    #[test]
    fn default_mirror_rewrites_docker_hub_images_only() {
        let vars = [("WSLC_REGISTRY_MIRROR", "mirror.example.com")];
        assert_eq!(
            resolve_image_reference_with("docker.io/library/alpine:latest", env(&vars)).unwrap(),
            "mirror.example.com/library/alpine:latest"
        );
        assert_eq!(
            resolve_image_reference_with("alpine:latest", env(&vars)).unwrap(),
            "mirror.example.com/alpine:latest"
        );
        assert_eq!(
            resolve_image_reference_with("ghcr.io/org/app:latest", env(&vars)).unwrap(),
            "ghcr.io/org/app:latest"
        );
    }

    #[test]
    fn per_registry_mirror_rewrites_matching_registry() {
        let vars = [
            ("WSLC_REGISTRY_MIRROR_GHCR_IO", "ghcr-mirror.example.com"),
            (
                "WSLC_REGISTRY_MIRROR_REGISTRY_EXAMPLE_COM_5000",
                "local-mirror",
            ),
        ];
        assert_eq!(
            resolve_image_reference_with("ghcr.io/org/app:latest", env(&vars)).unwrap(),
            "ghcr-mirror.example.com/org/app:latest"
        );
        assert_eq!(
            resolve_image_reference_with("registry.example.com:5000/ns/app:v1", env(&vars))
                .unwrap(),
            "local-mirror/ns/app:v1"
        );
    }

    #[test]
    fn rejects_url_style_mirror_values() {
        let vars = [("WSLC_REGISTRY_MIRROR", "https://mirror.example.com")];
        let err = resolve_image_reference_with("docker.io/library/alpine:latest", env(&vars))
            .unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }
}
