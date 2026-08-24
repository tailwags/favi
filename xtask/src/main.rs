use std::fs;

use anyhow::{Context, Result, bail};
use bindgen::callbacks::{DeriveInfo, ParseCallbacks};

/// Derives for plain-data types (no pointers into libavif-owned memory).
const DATA: &[&str] = &[
    "Copy",
    "Clone",
    "Hash",
    "PartialOrd",
    "Ord",
    "PartialEq",
    "Eq",
];

/// Same, minus the traits `f64` doesn't implement.
const FLOAT_DATA: &[&str] = &["Copy", "Clone", "PartialOrd", "PartialEq"];

/// For newtype enums: bindgen derives `Clone`/`Hash`/`PartialEq`/`Eq` on enums
/// unconditionally, so only the rest is listed.
const ENUM: &[&str] = &["Copy", "PartialOrd", "Ord"];

/// The only types that get any trait derived beyond `Debug`.
///
/// New libavif types land as `Debug`-only until reviewed and added here. A
/// wrong entry fails to compile; a missing one just costs a trait.
const ALLOWLIST: &[(&str, &[&str])] = &[
    // Enums.
    ("avifPlanesFlag", ENUM),
    ("avifChannelIndex", ENUM),
    ("avifResult", ENUM),
    ("avifHeaderFormat", ENUM),
    ("avifPixelFormat", ENUM),
    ("avifChromaSamplePosition", ENUM),
    ("avifRange", ENUM),
    ("avifTransformFlag", ENUM),
    ("avifSampleTransformRecipe", ENUM),
    ("avifRGBFormat", ENUM),
    ("avifChromaUpsampling", ENUM),
    ("avifChromaDownsampling", ENUM),
    ("avifCodecChoice", ENUM),
    ("avifCodecFlag", ENUM),
    ("avifStrictFlag", ENUM),
    ("avifDecoderSource", ENUM),
    ("avifProgressiveState", ENUM),
    ("avifImageContentTypeFlag", ENUM),
    ("avifAddImageFlag", ENUM),
    // Structs.
    ("avifPixelFormatInfo", DATA),
    ("avifDiagnostics", DATA),
    ("avifFraction", DATA),
    ("avifSignedFraction", DATA),
    ("avifUnsignedFraction", DATA),
    ("avifPixelAspectRatioBox", DATA),
    ("avifCleanApertureBox", DATA),
    ("avifImageRotation", DATA),
    ("avifImageMirror", DATA),
    ("avifCropRect", DATA),
    ("avifContentLightLevelInformationBox", DATA),
    ("avifIOStats", DATA),
    ("avifExtent", DATA),
    ("avifScalingMode", DATA),
    ("avifImageTiming", FLOAT_DATA),
];

const CICP_TYPES: &[(&str, &str)] = &[
    ("AVIF_COLOR_PRIMARIES_", "avifColorPrimaries"),
    (
        "AVIF_TRANSFER_CHARACTERISTICS_",
        "avifTransferCharacteristics",
    ),
    ("AVIF_MATRIX_COEFFICIENTS_", "avifMatrixCoefficients"),
];

#[derive(Debug)]
struct DeriveAllowlist;

impl ParseCallbacks for DeriveAllowlist {
    fn add_derives(&self, info: &DeriveInfo<'_>) -> Vec<String> {
        ALLOWLIST
            .iter()
            .find(|(name, _)| *name == info.name)
            .map(|(_, derives)| derives.iter().map(|s| (*s).to_owned()).collect())
            .unwrap_or_default()
    }
}

/// Rewrites the bindings so the anonymous CICP enums look like the named
/// enum newtypes and no `_bindgen_ty_*` identifiers remain. bindgen's output
/// format is fixed here, so plain string replacement does the job.
fn tidy_cicp_enums(bindings: &str) -> Result<String> {
    let mut out = bindings.to_owned();

    // Learn which typedef each anonymous newtype belongs to from its
    // constants: `pub const AVIF_COLOR_PRIMARIES_BT709: _bindgen_ty_1 = ...`.
    let mut nums: Vec<(&str, &str)> = Vec::new();
    for line in bindings.lines() {
        let Some(rest) = line.strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, rest)) = rest.split_once(": _bindgen_ty_") else {
            continue;
        };
        let Some((num, _)) = rest.split_once(' ') else {
            continue;
        };
        if let Some((_, ty)) = CICP_TYPES.iter().find(|(p, _)| name.starts_with(p))
            && !nums.iter().any(|(n, _)| *n == num)
        {
            nums.push((num, ty));
        }
    }

    for (num, ty) in nums {
        // Rename the anonymous newtype and retype it to the typedef's
        // underlying type. bindgen sized it from the C enum itself
        // (`c_uint`-wide); the typedef the ABI actually uses is narrower.
        // The extra derives also come from here: bindgen never calls
        // `add_derives` for anonymous types.
        let newtype = format!(
            "#[derive(Debug, Clone, Hash, PartialEq, Eq)]\n\
             pub struct _bindgen_ty_{num}(pub ::core::ffi::c_uint);"
        );

        let replacement = format!(
            "#[derive(Debug, Clone, Hash, PartialEq, Eq, Copy, PartialOrd, Ord)]\n\
             pub struct {ty}(pub u16);"
        );

        if !out.contains(&newtype) {
            bail!("anonymous enum _bindgen_ty_{num} not found in the generated bindings");
        }

        out = out.replace(&newtype, &replacement);

        // Retype the constants:
        // `pub const AVIF_COLOR_PRIMARIES_BT709: _bindgen_ty_1 = _bindgen_ty_1(1);`
        // becomes `...: avifColorPrimaries = avifColorPrimaries(1);`.
        let const_ty = format!(": _bindgen_ty_{num} = _bindgen_ty_{num}(");
        out = out.replace(&const_ty, &format!(": {ty} = {ty}("));
    }

    for (_, ty) in CICP_TYPES {
        // bindgen also emits an alias for the header's typedef; the renamed
        // newtype takes its place.
        let alias = format!("pub type {ty} = u16;\n");
        if !out.contains(&alias) {
            bail!("no typedef alias found for {ty} in the generated bindings");
        }
        out = out.replace(&alias, "");
    }

    if out.contains("_bindgen_ty_") {
        bail!(
            "generated bindings still contain _bindgen_ty_ identifiers; \
             tidy_cicp_enums needs updating for the new libavif header"
        );
    }

    Ok(out)
}

fn main() -> Result<()> {
    let bindings = bindgen::builder()
        .use_core()
        .generate_cstr(true)
        .derive_copy(false)
        .derive_hash(false)
        .derive_partialeq(false)
        .derive_eq(false)
        .derive_partialord(false)
        .derive_ord(false)
        .derive_default(false)
        .parse_callbacks(Box::new(DeriveAllowlist))
        .default_enum_style(bindgen::EnumVariation::NewType {
            is_bitfield: false,
            is_global: false,
        })
        .prepend_enum_name(false)
        .sort_semantically(true)
        .layout_tests(false)
        .allowlist_item("avif.*")
        .allowlist_item("AVIF.*")
        .header("libavif/include/avif/avif.h")
        .generate()
        .context("Failed to generate bindings")?
        .to_string();

    let bindings = tidy_cicp_enums(&bindings)?;

    fs::write("src/sys.rs", bindings).context("Couldn't write bindings")?;

    Ok(())
}
