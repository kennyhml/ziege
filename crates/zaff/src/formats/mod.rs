mod clas;
mod dtel;
mod prog;

pub use clas::{
    AffClass, AffClassAbapLanguageVersion, AffClassCategory, AffClassDescriptions, AffClassHeader,
    AffEventDescription, AffMethodDescription, AffNameDescription, CLASS_FORMAT,
};
pub use dtel::{
    AffAbapLanguageVersion, AffBasicDirection, AffBidirectionalOptions, AffDataElement,
    AffDataElementAdditionalProperties, AffDataElementCategory, AffDataElementFieldLabels,
    AffDataElementHeader, AffDataElementTypeInformation, AffPredefinedType, AffSearchHelp,
    DATA_ELEMENT_FORMAT,
};
pub use prog::{
    AffLogicalDatabase, AffProgram, AffProgramGeneralInformation, AffProgramHeader,
    AffProgramStatus, AffProgramType, PROGRAM_FORMAT,
};

pub(crate) use clas::ClassDescriptor;
pub(crate) use dtel::DataElementDescriptor;
pub(crate) use prog::ProgramDescriptor;
