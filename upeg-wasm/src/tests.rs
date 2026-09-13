use super::*;
use upeg_core::{
    ChoiceOption, ChoiceSpec, InputKind, Invoker, OutputKind, PegboardUnits, PinKind, Surface,
};
use upeg_plugin_api::{
    PluginChoiceOption, PluginFileInputPolicy, PluginInputField, PluginInputKind, PluginInputSpec,
    PluginOutputField, PluginOutputKind, PluginOutputSpec,
};

mod decl_basics;
mod decl_edge_cases;
mod decl_identity;
mod file_input_policy;
mod input_output_spec;
mod inspect_bytes;
mod kind_parity;
