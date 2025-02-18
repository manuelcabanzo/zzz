pub trait Plugin {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn activate(&self);
    fn deactivate(&self);
}
