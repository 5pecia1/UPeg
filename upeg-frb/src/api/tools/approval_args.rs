//! GUI-only approval shaping, kept beside the tool wire surface but outside
//! its DTO inventory so dispatch policy remains independently readable.

pub(super) const APPROVE_RESERVED_ARG: &str = "approve";

/// Make approval exclusively owned by the typed GUI flag. Preset/deep-link
/// data cannot retain either whole-call approval or individual approved steps.
pub(super) fn shape_approval_arg(mut args: serde_json::Value, approve: bool) -> serde_json::Value {
    let Some(object) = args.as_object_mut() else {
        return args;
    };
    object.remove(APPROVE_RESERVED_ARG);
    if let Some(context) = object
        .get_mut(upeg_core::EXECUTION_CONTEXT_ARG)
        .and_then(serde_json::Value::as_object_mut)
    {
        context.remove(upeg_core::EXECUTION_CONTEXT_APPROVED_STEPS);
    }
    if approve {
        object.insert(
            APPROVE_RESERVED_ARG.to_string(),
            serde_json::Value::Bool(true),
        );
    }
    args
}
