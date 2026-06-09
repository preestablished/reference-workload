#![forbid(unsafe_code)]

pub fn sample_experiment_spec() -> determinism_proto::controlplane::v1::ExperimentSpec {
    determinism_proto::controlplane::v1::ExperimentSpec {
        seed: 1,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_proto_client_compiles() {
        assert_eq!(super::sample_experiment_spec().seed, 1);
    }
}
