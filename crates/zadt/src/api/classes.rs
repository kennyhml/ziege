use super::run::ObjectRun;
use crate::{
    CategoryId, Discovery, EncodeError, EncodedOperation, Operation, OperationResponse,
    RequiresDiscovery, ResponseError, Stateless,
    objects::{Class, ImmediateRun, ObjectKey, ObjectRef, ObjectSnapshot, RunCapability},
};

/// The plain-text console output produced by running an ABAP class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassRunResult {
    /// The class that was executed.
    pub reference: ObjectKey<Class>,

    /// The rendered class-run output returned by SAP.
    pub content: String,
}

impl ClassRunResult {
    pub(crate) fn new(reference: ObjectKey<Class>, content: String) -> Self {
        Self { reference, content }
    }
}

/// Runs an ABAP class and returns its rendered console output.
#[derive(Debug)]
pub struct ClassRun {
    class: ObjectKey<Class>,
    run: ObjectRun,
}

impl ClassRun {
    const NAME_VARIABLE: &str = "classname";
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/oo",
        term: "classrun",
    };
    const RELATION: &str = "http://www.sap.com/adt/relations/oo/classrun";

    fn new(class: ObjectKey<Class>) -> Self {
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
    const RUN: RunCapability = RunCapability::new(
        ClassRun::CATEGORY,
        ClassRun::RELATION,
        ClassRun::NAME_VARIABLE,
    );
}

impl Operation for ClassRun {
    type Response = ClassRunResult;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        self.run.encode(resolver)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        let output = self.run.decode(response)?;
        Ok(ClassRunResult::new(self.class.clone(), output.content))
    }
}

impl ObjectKey<Class> {
    /// Creates an operation that runs this class.
    pub fn run(&self) -> ClassRun {
        ClassRun::new(self.clone())
    }
}

impl ObjectSnapshot<Class> {
    /// Creates an operation that runs this loaded class.
    pub fn run(&self) -> ClassRun {
        self.reference().run()
    }
}

impl ObjectRef<Class> {
    /// Creates a name-based run through the advertised class-run template.
    pub fn run(&self) -> ClassRun {
        self.key().run()
    }
}
