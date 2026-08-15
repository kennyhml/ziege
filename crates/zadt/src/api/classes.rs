use super::run::ObjectRun;
use crate::{
    AdtRequest, Client, Operation, OperationError, OperationResponse, Ready, ResponseError,
    Stateless,
    objects::{Class, ImmediateRun, ObjectRef, RunCapability},
    vocabulary::CategoryId,
};

const CLASS_NAME_VARIABLE: &str = "classname";
const CLASS_RUN_CATEGORY: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/oo",
    term: "classrun",
};
const CLASS_RUN_RELATION: &str = "http://www.sap.com/adt/relations/oo/classrun";

/// The plain-text console output produced by running an ABAP class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassRunResult {
    /// The class that was executed.
    pub reference: ObjectRef<Class>,

    /// The rendered class-run output returned by SAP.
    pub content: String,
}

impl ClassRunResult {
    pub(crate) fn new(reference: ObjectRef<Class>, content: String) -> Self {
        Self { reference, content }
    }
}

/// Runs an ABAP class and returns its rendered console output.
#[derive(Debug)]
pub struct ClassRun {
    class: ObjectRef<Class>,
    run: ObjectRun,
}

impl ClassRun {
    fn new(class: ObjectRef<Class>) -> Self {
        let run = ObjectRun::typed(&class);
        Self { class, run }
    }

    /// Runs the class with the supplied ABAP profiler trace identifier.
    #[must_use]
    pub fn profiler_id(mut self, profiler_id: impl Into<String>) -> Self {
        self.run = self.run.profiler_id(profiler_id);
        self
    }
}

impl ImmediateRun for Class {
    const RUN: RunCapability =
        RunCapability::new(CLASS_RUN_CATEGORY, CLASS_RUN_RELATION, CLASS_NAME_VARIABLE);
}

impl Operation<Ready> for ClassRun {
    type Response = ClassRunResult;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        self.run.request(client)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        let output = self.run.decode(response)?;
        Ok(ClassRunResult::new(self.class.clone(), output.content))
    }
}

impl ObjectRef<Class> {
    /// Creates an operation that runs this class.
    pub fn run(&self) -> ClassRun {
        ClassRun::new(self.clone())
    }
}
