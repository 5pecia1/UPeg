use super::{
    FilePolicySchemaError, FileWireSchemaError, InputName, InputSpecError, InputValueError,
};

/// Errors raised while adapting static metadata or draft form state.
//
// `Eq` dropped to match `InputValueError` (carries `f64` for OutOfRange).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum InputAdapterError {
    #[error(transparent)]
    Spec(#[from] InputSpecError),
    #[error(transparent)]
    Value(#[from] InputValueError),
    #[error("form field `{name}` is not declared by the input spec")]
    UnknownFormField { name: InputName },
    #[error("form field `{name}` appears more than once")]
    DuplicateFormField { name: InputName },
    #[error("form field `{name}` expected {expected} draft, got {actual}")]
    DraftKindMismatch {
        name: InputName,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("form field `{name}` contains invalid number `{value}`")]
    InvalidNumber { name: InputName, value: String },
    #[error("form field `{name}` contains invalid integer `{value}`")]
    InvalidInteger { name: InputName, value: String },
    #[error("form field `{name}` contains invalid json: {detail}")]
    InvalidJson { name: InputName, detail: String },
    #[error("JSON Schema root must be an object, got {kind}")]
    JsonSchemaRootNotObject { kind: &'static str },
    #[error("JSON Schema root type must be `object`, got `{ty}`")]
    JsonSchemaRootTypeNotObject { ty: String },
    #[error("JSON Schema root type must be a string, got {kind}")]
    JsonSchemaRootTypeNotString { kind: &'static str },
    #[error("JSON Schema properties must be an object, got {kind}")]
    JsonSchemaPropertiesNotObject { kind: &'static str },
    #[error("JSON Schema property `{property}` must be an object")]
    JsonSchemaPropertyNotObject { property: String },
    #[error("JSON Schema property `{property}` type must be a string, got {kind}")]
    JsonSchemaTypeNotString {
        property: String,
        kind: &'static str,
    },
    #[error("JSON Schema property `{property}` uses unsupported type `{ty}`")]
    JsonSchemaUnsupportedType { property: String, ty: String },
    #[error("JSON Schema property `{property}` uses unsupported nested object schema")]
    JsonSchemaNestedObject { property: String },
    #[error("JSON Schema {location} uses unsupported keyword `{keyword}`")]
    JsonSchemaUnsupportedKeyword {
        location: String,
        keyword: &'static str,
    },
    #[error("JSON Schema required must be an array, got {kind}")]
    JsonSchemaRequiredNotArray { kind: &'static str },
    #[error("JSON Schema required entries must be strings, got {kind}")]
    JsonSchemaRequiredEntryNotString { kind: &'static str },
    #[error("JSON Schema required field `{property}` is not declared in properties")]
    JsonSchemaRequiredPropertyNotDeclared { property: String },
    #[error("JSON Schema additionalProperties must be false, got {kind}")]
    JsonSchemaAdditionalPropertiesNotFalse { kind: &'static str },
    #[error("JSON Schema property `{property}` description must be a string, got {kind}")]
    JsonSchemaDescriptionNotString {
        property: String,
        kind: &'static str,
    },
    #[error("JSON Schema property `{property}` title must be a string, got {kind}")]
    JsonSchemaTitleNotString {
        property: String,
        kind: &'static str,
    },
    #[error("JSON Schema property `{property}` enum must be an array, got {kind}")]
    JsonSchemaEnumNotArray {
        property: String,
        kind: &'static str,
    },
    #[error("JSON Schema property `{property}` enum must contain at least one value")]
    JsonSchemaEmptyEnum { property: String },
    #[error("JSON Schema property `{property}` enum values must be strings, got {kind}")]
    JsonSchemaEnumValueNotString {
        property: String,
        kind: &'static str,
    },
    #[error("JSON Schema property `{property}` enum is unsupported for type `{ty}`")]
    JsonSchemaEnumUnsupportedType { property: String, ty: String },
    #[error("JSON Schema property `{property}` array items must be an object, got {kind}")]
    JsonSchemaArrayItemsNotObject {
        property: String,
        kind: &'static str,
    },
    #[error("JSON Schema property `{property}` uses unsupported tuple array items")]
    JsonSchemaTupleArray { property: String },
    #[error("JSON Schema property `{property}` array items type must be a string, got {kind}")]
    JsonSchemaArrayItemsTypeNotString {
        property: String,
        kind: &'static str,
    },
    #[error("JSON Schema property `{property}` array items must have type `string`, got `{ty}`")]
    JsonSchemaArrayItemsUnsupportedType { property: String, ty: String },
    #[error("JSON Schema property `{property}` array choices must declare string enum items")]
    JsonSchemaArrayItemsMissingEnum { property: String },
    #[error("JSON Schema property `{property}` options must declare a string enum")]
    JsonSchemaOptionsMissingEnum { property: String },
    #[error(
        "JSON Schema property `{property}` numeric constraint `{keyword}` must be a number, got {kind}"
    )]
    JsonSchemaNumericConstraintNotNumber {
        property: String,
        keyword: &'static str,
        kind: &'static str,
    },
    #[error(
        "JSON Schema property `{property}` numeric constraint `{keyword}` is unsupported for input kind `{kind}`"
    )]
    JsonSchemaNumericConstraintUnsupportedKind {
        property: String,
        keyword: &'static str,
        kind: &'static str,
    },
    #[error(
        "JSON Schema property `{property}` string constraint `{keyword}` must be a string, got {kind}"
    )]
    JsonSchemaStringConstraintNotString {
        property: String,
        keyword: &'static str,
        kind: &'static str,
    },
    #[error(
        "JSON Schema property `{property}` string constraint `{keyword}` is unsupported for input kind `{kind}`"
    )]
    JsonSchemaStringConstraintUnsupportedKind {
        property: String,
        keyword: &'static str,
        kind: &'static str,
    },
    #[error("JSON Schema property `{property}` has invalid file policy: {source}")]
    JsonSchemaFilePolicy {
        property: String,
        #[source]
        source: FilePolicySchemaError,
    },
    #[error("JSON Schema property `{property}` has invalid file wire contract: {source}")]
    JsonSchemaFileWire {
        property: String,
        #[source]
        source: FileWireSchemaError,
    },
}
