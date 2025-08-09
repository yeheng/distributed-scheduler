
use opentelemetry::global;
use opentelemetry::propagation::{Extractor, Injector};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub struct CrossComponentTracer;

impl CrossComponentTracer {
    pub fn extract_trace_context_from_headers(
        headers: &std::collections::HashMap<String, String>,
    ) -> Option<opentelemetry::Context> {
        struct HeaderExtractor<'a>(&'a std::collections::HashMap<String, String>);

        impl<'a> Extractor for HeaderExtractor<'a> {
            fn get(&self, key: &str) -> Option<&str> {
                self.0.get(key).map(|s| s.as_str())
            }

            fn keys(&self) -> Vec<&str> {
                self.0.keys().map(|s| s.as_str()).collect()
            }
        }

        let extractor = HeaderExtractor(headers);
        let context = global::get_text_map_propagator(|propagator| propagator.extract(&extractor));

        Some(context)
    }
    pub fn inject_trace_context_into_headers(
        headers: &mut std::collections::HashMap<String, String>,
    ) {
        struct HeaderInjector<'a>(&'a mut std::collections::HashMap<String, String>);

        impl<'a> Injector for HeaderInjector<'a> {
            fn set(&mut self, key: &str, value: String) {
                self.0.insert(key.to_string(), value);
            }
        }

        let mut injector = HeaderInjector(headers);
        let context = tracing::Span::current().context();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&context, &mut injector)
        });
    }
    pub fn create_child_span_from_context(
        context: opentelemetry::Context,
        span_name: &str,
        attributes: Vec<(String, String)>,
    ) -> tracing::Span {
        let span = tracing::info_span!(target: "scheduler", "{}", span_name);
        span.set_parent(context);
        for (key, value) in attributes {
            span.set_attribute(key, value);
        }

        span
    }
}
