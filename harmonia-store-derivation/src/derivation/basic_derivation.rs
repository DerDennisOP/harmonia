use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use proptest::prelude::{Arbitrary, BoxedStrategy};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::derived_path::SingleDerivedPath;
use harmonia_store_path::{StorePathName, StorePathSet};

use super::{DerivationInputs, DerivationOutput, DerivationOutputs};

/// Structured attributes for a derivation.
///
/// When present, derivation attributes are passed to the builder as a JSON object
/// via the `__json` environment variable, rather than individual environment variables.
/// This allows passing complex data structures (arrays, nested objects, etc.) to builders.
///
/// Note: The `env` map must not contain the key `__json` when structured attrs are used,
/// as that key is reserved for encoding the structured attributes themselves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructuredAttrs {
    structured_attributes: serde_json::Map<String, serde_json::Value>,
    /// The exact JSON text of the `__json` environment variable. Printing writes this text
    /// unchanged, since Nix hashes the derivation file as written. Parsing and serializing
    /// the JSON a second time changes the appearance of floating-point numbers, so `1e-06`
    /// can become `1e-6`. The computed output paths then fail to match.
    json_env_entry_text: String,
}

impl StructuredAttrs {
    pub fn from_json_text_keeping_original_text(
        json_env_entry_text: String,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            structured_attributes: serde_json::from_str(&json_env_entry_text)?,
            json_env_entry_text,
        })
    }

    pub fn structured_attributes(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.structured_attributes
    }

    pub fn structured_attributes_json_text(&self) -> &str {
        &self.json_env_entry_text
    }
}

impl From<serde_json::Map<String, serde_json::Value>> for StructuredAttrs {
    fn from(structured_attributes: serde_json::Map<String, serde_json::Value>) -> Self {
        let json_env_entry_text =
            serde_json::Value::Object(structured_attributes.clone()).to_string();
        Self {
            structured_attributes,
            json_env_entry_text,
        }
    }
}

impl Serialize for StructuredAttrs {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.structured_attributes.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StructuredAttrs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde_json::Map::deserialize(deserializer).map(Self::from)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DerivationT<Inputs, Output = DerivationOutput> {
    /// The name of the derivation
    pub name: StorePathName,
    pub outputs: DerivationOutputs<Output>,
    pub inputs: Inputs,
    pub platform: bytes::Bytes,
    pub builder: bytes::Bytes,
    pub args: Vec<bytes::Bytes>,
    /// Environment variables passed to the builder.
    ///
    /// Note: Must not contain the key `__json` as that key is reserved
    /// for encoding structured attributes.
    pub env: BTreeMap<bytes::Bytes, bytes::Bytes>,
    /// Optional structured attributes.
    ///
    /// When present, attributes are passed to the builder as JSON via the `__json`
    /// environment variable instead of individual environment variables.
    pub structured_attrs: Option<StructuredAttrs>,
}

fn default_version() -> u32 {
    4
}

pub type BasicDerivation = DerivationT<StorePathSet>;
pub type Derivation = DerivationT<BTreeSet<SingleDerivedPath>>;

#[cfg(any(test, feature = "test"))]
pub mod arbitrary {
    use super::*;
    use crate::{
        derivation::derivation_output::arbitrary::arb_derivation_outputs,
        test::arbitrary::arb_byte_string,
    };
    use ::proptest::prelude::*;
    use proptest::sample::SizeRange;

    impl Arbitrary for BasicDerivation {
        type Parameters = ();
        type Strategy = BoxedStrategy<BasicDerivation>;

        fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
            arb_basic_derivation().boxed()
        }
    }

    prop_compose! {
        pub fn arb_basic_derivation()
        (
            name in any::<StorePathName>(),
            outputs in arb_derivation_outputs(1..15),
            inputs in any::<StorePathSet>(),
            platform in arb_byte_string(),
            builder in arb_byte_string(),
            args in proptest::collection::vec(arb_byte_string(), SizeRange::default()),
            env in proptest::collection::btree_map(arb_byte_string(), arb_byte_string(), SizeRange::default()),
            structured_attrs in proptest::option::of(arb_structured_attrs())
        ) -> BasicDerivation
        {
            DerivationT {
                name,
                outputs,
                inputs,
                platform,
                builder,
                args,
                env,
                structured_attrs,
            }
        }
    }

    fn arb_structured_attrs() -> impl Strategy<Value = StructuredAttrs> {
        // Generate a simple JSON object with string keys and various JSON values
        proptest::collection::btree_map(
            any::<String>(),
            prop_oneof![
                any::<String>().prop_map(serde_json::Value::String),
                any::<i64>().prop_map(|i| serde_json::Value::Number(i.into())),
                any::<bool>().prop_map(serde_json::Value::Bool),
                Just(serde_json::Value::Null),
            ],
            0..10,
        )
        .prop_map(|map| {
            let mut attrs = serde_json::Map::new();
            for (k, v) in map {
                attrs.insert(k, v);
            }
            StructuredAttrs::from(attrs)
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DerivationHelperT<Inputs> {
    name: String,
    outputs: DerivationOutputs,
    inputs: Inputs,
    #[serde(rename = "system")]
    platform: String,
    builder: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    structured_attrs: Option<StructuredAttrs>,
    #[serde(default = "default_version")]
    version: u32,
}

/// Trait for converting between the in-memory input representation and
/// the JSON-serializable form.
trait SerdeInputs: Sized {
    type Helper: Serialize + for<'de> Deserialize<'de>;
    fn to_helper(&self) -> Self::Helper;
    fn from_helper(h: Self::Helper) -> Self;
}

impl SerdeInputs for StorePathSet {
    type Helper = StorePathSet;
    fn to_helper(&self) -> StorePathSet {
        self.clone()
    }
    fn from_helper(h: StorePathSet) -> Self {
        h
    }
}

impl SerdeInputs for BTreeSet<SingleDerivedPath> {
    type Helper = DerivationInputs;
    fn to_helper(&self) -> DerivationInputs {
        DerivationInputs::from(self)
    }
    fn from_helper(h: DerivationInputs) -> Self {
        BTreeSet::from(&h)
    }
}

fn derivation_to_helper<Inputs: SerdeInputs>(
    drv: &DerivationT<Inputs>,
) -> Result<DerivationHelperT<Inputs::Helper>, std::str::Utf8Error> {
    let platform_str = std::str::from_utf8(&drv.platform)?;
    let builder_str = std::str::from_utf8(&drv.builder)?;
    let args_strs: Result<Vec<String>, _> = drv
        .args
        .iter()
        .map(|b| std::str::from_utf8(b).map(|s| s.to_string()))
        .collect();
    let args_strs = args_strs?;
    let env_map: Result<BTreeMap<String, String>, _> = drv
        .env
        .iter()
        .map(|(k, v)| {
            Ok((
                std::str::from_utf8(k)?.to_string(),
                std::str::from_utf8(v)?.to_string(),
            ))
        })
        .collect();
    let env_map = env_map?;

    Ok(DerivationHelperT {
        name: drv.name.to_string(),
        version: default_version(),
        outputs: drv.outputs.clone(),
        inputs: drv.inputs.to_helper(),
        platform: platform_str.to_string(),
        builder: builder_str.to_string(),
        args: args_strs,
        env: env_map,
        structured_attrs: drv.structured_attrs.clone(),
    })
}

fn helper_to_derivation<'de, Inputs: SerdeInputs, D: Deserializer<'de>>(
    helper: DerivationHelperT<Inputs::Helper>,
) -> Result<DerivationT<Inputs>, D::Error> {
    if helper.version != 4 {
        return Err(serde::de::Error::custom(format!(
            "unsupported derivation version: {}, expected 4",
            helper.version
        )));
    }
    Ok(DerivationT {
        name: helper
            .name
            .parse()
            .map_err(|e| serde::de::Error::custom(format!("invalid derivation name: {}", e)))?,
        outputs: helper.outputs,
        inputs: Inputs::from_helper(helper.inputs),
        platform: bytes::Bytes::from(helper.platform),
        builder: bytes::Bytes::from(helper.builder),
        args: helper.args.into_iter().map(bytes::Bytes::from).collect(),
        env: helper
            .env
            .into_iter()
            .map(|(k, v)| (bytes::Bytes::from(k), bytes::Bytes::from(v)))
            .collect(),
        structured_attrs: helper.structured_attrs,
    })
}

impl Serialize for BasicDerivation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        derivation_to_helper(self)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BasicDerivation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = DerivationHelperT::<StorePathSet>::deserialize(deserializer)?;
        helper_to_derivation::<StorePathSet, D>(helper)
    }
}

impl Serialize for Derivation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        derivation_to_helper(self)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Derivation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = DerivationHelperT::<DerivationInputs>::deserialize(deserializer)?;
        helper_to_derivation::<BTreeSet<SingleDerivedPath>, D>(helper)
    }
}

impl<Inputs, Output> DerivationT<Inputs, Output> {
    /// Transform outputs, keeping everything else the same.
    pub fn map_outputs<T>(self, mut f: impl FnMut(Output) -> T) -> DerivationT<Inputs, T> {
        DerivationT {
            name: self.name,
            outputs: self.outputs.into_iter().map(|(k, v)| (k, f(v))).collect(),
            inputs: self.inputs,
            platform: self.platform,
            builder: self.builder,
            args: self.args,
            env: self.env,
            structured_attrs: self.structured_attrs,
        }
    }

    /// Convert the inputs to the split-by-kind format representation
    /// ([`DerivationInputs`]), moving the other fields. This is the
    /// boundary conversion for the on-disk/wire formats (e.g. the ATerm
    /// printer).
    pub fn into_full(self) -> DerivationT<DerivationInputs, Output>
    where
        for<'a> DerivationInputs: From<&'a Inputs>,
    {
        self.map_inputs(|i| DerivationInputs::from(&i))
    }

    /// Transform inputs, keeping everything else the same.
    pub fn map_inputs<T>(self, f: impl FnOnce(Inputs) -> T) -> DerivationT<T, Output> {
        DerivationT {
            name: self.name,
            outputs: self.outputs,
            inputs: f(self.inputs),
            platform: self.platform,
            builder: self.builder,
            args: self.args,
            env: self.env,
            structured_attrs: self.structured_attrs,
        }
    }
}

impl<Inputs> DerivationT<Inputs> {
    /// Replace all occurrences of each rewrite's key with its value in builder,
    /// args, env (keys and values), and structured_attrs.
    ///
    /// This is used during derivation resolution to substitute CA output
    /// placeholders with their actual store paths.
    pub fn apply_rewrites(&mut self, rewrites: &BTreeMap<bytes::Bytes, bytes::Bytes>) {
        if rewrites.is_empty() {
            return;
        }

        fn rewrite(
            s: &bytes::Bytes,
            rewrites: &BTreeMap<bytes::Bytes, bytes::Bytes>,
        ) -> bytes::Bytes {
            let mut buf = Vec::from(s.as_ref());
            for (from, to) in rewrites {
                assert!(!from.is_empty(), "rewrite key must not be empty");
                // Repeatedly scan for the pattern and replace all occurrences.
                // TODO: after https://github.com/tokio-rs/bytes/issues/824 is resolved, use the replace method on BytesMut.
                let mut i = 0;
                while i + from.len() <= buf.len() {
                    if buf[i..i + from.len()] == **from {
                        buf.splice(i..i + from.len(), to.iter().copied());
                        i += to.len();
                    } else {
                        i += 1;
                    }
                }
            }
            bytes::Bytes::from(buf)
        }

        self.builder = rewrite(&self.builder, rewrites);

        for arg in &mut self.args {
            *arg = rewrite(arg, rewrites);
        }

        let old_env = std::mem::take(&mut self.env);
        for (k, v) in old_env {
            self.env
                .insert(rewrite(&k, rewrites), rewrite(&v, rewrites));
        }

        if let Some(ref mut sa) = self.structured_attrs {
            let json_bytes =
                bytes::Bytes::copy_from_slice(sa.structured_attributes_json_text().as_bytes());
            let rewritten = rewrite(&json_bytes, rewrites);
            if let Some(rewritten) = String::from_utf8(rewritten.to_vec())
                .ok()
                .and_then(|text| StructuredAttrs::from_json_text_keeping_original_text(text).ok())
            {
                *sa = rewritten;
            }
        }
    }
}

impl Derivation {
    /// Create a new derivation with the given name, platform, and builder, which are the minimal
    /// fields for a derivation.
    pub fn new(name: StorePathName, platform: bytes::Bytes, builder: bytes::Bytes) -> Self {
        Self {
            name,
            outputs: DerivationOutputs::new(),
            inputs: BTreeSet::new(),
            platform,
            builder,
            args: Vec::new(),
            env: Default::default(),
            structured_attrs: None,
        }
    }
}
