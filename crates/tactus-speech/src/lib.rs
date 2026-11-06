use prismer::{Backend, Prism};

pub struct Speech {
    backend: Option<Backend<'static>>,
}

impl Speech {
    pub fn new() -> Self {
        let backend = Prism::new().ok().and_then(|prism| {
            let prism: &'static Prism = Box::leak(Box::new(prism));
            prism.create_best().ok()
        });
        Self { backend }
    }

    pub fn backend_name(&self) -> Option<String> {
        self.backend.as_ref().map(Backend::name)
    }

    pub fn say(&self, text: &str, interrupt: bool) {
        if let (Some(backend), false) = (&self.backend, text.is_empty()) {
            let _ = backend.output(text, interrupt);
        }
    }
}
