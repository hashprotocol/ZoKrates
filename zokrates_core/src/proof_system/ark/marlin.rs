use ark_marlin::{IndexProverKey, IndexVerifierKey, Proof as ArkProof};

use ark_marlin::Marlin as ArkMarlin;

use ark_ec::PairingEngine;
use ark_poly::univariate::DensePolynomial;
use ark_poly_commit::marlin_pc::MarlinKZG10;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use sha2::Sha256;

use zokrates_field::{ArkFieldExtensions, Field};

use crate::ir::{Prog, Witness};
use crate::proof_system::ark::parse_fr;
use crate::proof_system::ark::Ark;
use crate::proof_system::ark::Computation;
use crate::proof_system::marlin::{self, ProofPoints, VerificationKey};
use crate::proof_system::Scheme;
use crate::proof_system::{Backend, Proof, SetupKeypair, UniversalBackend};

impl<T: Field + ArkFieldExtensions> UniversalBackend<T, marlin::Marlin> for Ark {
    fn universal_setup(size: u32) -> Vec<u8> {
        use rand_0_7::SeedableRng;

        let rng = &mut rand_0_7::rngs::StdRng::from_entropy();

        let srs = ArkMarlin::<
            <<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr,
            MarlinKZG10<
                T::ArkEngine,
                DensePolynomial<<<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr>,
            >,
            Sha256,
        >::universal_setup(
            2usize.pow(size), 2usize.pow(size), 2usize.pow(size), rng
        )
        .unwrap();

        let mut res = vec![];

        srs.serialize(&mut res).unwrap();

        res
    }

    fn setup(
        universal_srs: Vec<u8>,
        program: Prog<T>,
    ) -> Result<SetupKeypair<<marlin::Marlin as Scheme<T>>::VerificationKey>, String> {
        let computation = Computation::without_witness(program);

        let srs = ark_marlin::UniversalSRS::<
            <<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr,
            MarlinKZG10<
                T::ArkEngine,
                DensePolynomial<<<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr>,
            >,
        >::deserialize(&mut universal_srs.as_slice())
        .unwrap();

        let (pk, vk) = ArkMarlin::<
            <<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr,
            MarlinKZG10<
                T::ArkEngine,
                DensePolynomial<<<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr>,
            >,
            Sha256,
        >::index(&srs, computation)
        .map_err(|e| match e {
            ark_marlin::Error::IndexTooLarge => String::from("The universal setup is too small for this program, please provide a larger universal setup"),
            _ => String::from("Unknown error specializing the universal setup for this program")
        })?;

        let mut serialized_pk: Vec<u8> = Vec::new();
        pk.serialize_uncompressed(&mut serialized_pk).unwrap();

        let mut serialized_vk: Vec<u8> = Vec::new();
        vk.serialize_uncompressed(&mut serialized_vk).unwrap();

        Ok(SetupKeypair::new(
            VerificationKey { raw: serialized_vk },
            serialized_pk,
        ))
    }
}

impl<T: Field + ArkFieldExtensions> Backend<T, marlin::Marlin> for Ark {
    fn generate_proof(
        program: Prog<T>,
        witness: Witness<T>,
        proving_key: Vec<u8>,
    ) -> Proof<<marlin::Marlin as Scheme<T>>::ProofPoints> {
        let computation = Computation::with_witness(program, witness);

        use rand_0_7::SeedableRng;

        let rng = &mut rand_0_7::rngs::StdRng::from_entropy();

        let pk = IndexProverKey::<
            <<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr,
            MarlinKZG10<
                T::ArkEngine,
                DensePolynomial<<<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr>,
            >,
        >::deserialize_uncompressed(&mut proving_key.as_slice())
        .unwrap();

        let inputs = computation
            .public_inputs_values()
            .iter()
            .map(parse_fr::<T>)
            .collect::<Vec<_>>();

        let proof = ArkMarlin::<
            <<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr,
            MarlinKZG10<
                T::ArkEngine,
                DensePolynomial<<<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr>,
            >,
            Sha256,
        >::prove(&pk, computation, rng)
        .unwrap();

        let mut serialized_proof: Vec<u8> = Vec::new();
        proof.serialize_uncompressed(&mut serialized_proof).unwrap();

        Proof::new(
            ProofPoints {
                raw: serialized_proof,
            },
            inputs,
        )
    }

    fn verify(
        vk: <marlin::Marlin as Scheme<T>>::VerificationKey,
        proof: Proof<<marlin::Marlin as Scheme<T>>::ProofPoints>,
    ) -> bool {
        let inputs: Vec<_> = proof
            .inputs
            .iter()
            .map(|s| {
                T::try_from_str(s.trim_start_matches("0x"), 16)
                    .unwrap()
                    .into_ark()
            })
            .collect::<Vec<_>>();

        let proof = ArkProof::<
            <<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr,
            MarlinKZG10<
                T::ArkEngine,
                DensePolynomial<<<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr>,
            >,
        >::deserialize_uncompressed(&mut proof.proof.raw.as_slice())
        .unwrap();

        let vk = IndexVerifierKey::<
            <<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr,
            MarlinKZG10<
                T::ArkEngine,
                DensePolynomial<<<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr>,
            >,
        >::deserialize_uncompressed(&mut vk.raw.as_slice())
        .unwrap();

        use rand_0_7::SeedableRng;

        let rng = &mut rand_0_7::rngs::StdRng::from_entropy();

        ArkMarlin::<
            <<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr,
            MarlinKZG10<
                T::ArkEngine,
                DensePolynomial<<<T as ArkFieldExtensions>::ArkEngine as PairingEngine>::Fr>,
            >,
            Sha256,
        >::verify(&vk, &inputs, &proof, rng)
        .unwrap()
    }
}

pub mod serialization {
    use crate::proof_system::{G1Affine, G2Affine, G2AffineFq};
    use ark_ec::PairingEngine;
    use ark_ff::FromBytes;
    use zokrates_field::ArkFieldExtensions;

    #[inline]
    fn decode_hex(value: String) -> Vec<u8> {
        let mut bytes = hex::decode(value.strip_prefix("0x").unwrap()).unwrap();
        bytes.reverse();
        bytes
    }

    pub fn to_g1<T: ArkFieldExtensions>(g1: G1Affine) -> <T::ArkEngine as PairingEngine>::G1Affine {
        let mut bytes = vec![];
        bytes.append(&mut decode_hex(g1.0));
        bytes.append(&mut decode_hex(g1.1));
        bytes.push(0u8); // infinity flag

        <T::ArkEngine as PairingEngine>::G1Affine::read(&*bytes).unwrap()
    }

    pub fn to_g2<T: ArkFieldExtensions>(g2: G2Affine) -> <T::ArkEngine as PairingEngine>::G2Affine {
        let mut bytes = vec![];
        bytes.append(&mut decode_hex((g2.0).0));
        bytes.append(&mut decode_hex((g2.0).1));
        bytes.append(&mut decode_hex((g2.1).0));
        bytes.append(&mut decode_hex((g2.1).1));
        bytes.push(0u8); // infinity flag

        <T::ArkEngine as PairingEngine>::G2Affine::read(&*bytes).unwrap()
    }

    pub fn to_g2_fq<T: ArkFieldExtensions>(
        g2: G2AffineFq,
    ) -> <T::ArkEngine as PairingEngine>::G2Affine {
        let mut bytes = vec![];
        bytes.append(&mut decode_hex(g2.0));
        bytes.append(&mut decode_hex(g2.1));
        bytes.push(0u8); // infinity flag

        <T::ArkEngine as PairingEngine>::G2Affine::read(&*bytes).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::flat_absy::FlatVariable;
    use crate::ir::{Function, Interpreter, Prog, QuadComb, Statement};

    use super::*;
    use crate::proof_system::scheme::Marlin;
    use zokrates_field::{Bls12_377Field, Bw6_761Field};

    #[test]
    fn verify_bls12_377_field() {
        let program: Prog<Bls12_377Field> = Prog {
            main: Function {
                id: String::from("main"),
                arguments: vec![FlatVariable::new(0)],
                returns: vec![FlatVariable::public(0)],
                statements: vec![
                    Statement::Constraint(
                        QuadComb::from_linear_combinations(
                            FlatVariable::new(0).into(),
                            FlatVariable::new(0).into(),
                        ),
                        FlatVariable::new(1).into(),
                    ),
                    Statement::Constraint(
                        FlatVariable::new(1).into(),
                        FlatVariable::public(0).into(),
                    ),
                ],
            },
            private: vec![true],
        };

        let srs = <Ark as UniversalBackend<Bls12_377Field, Marlin>>::universal_setup(5);
        let keypair =
            <Ark as UniversalBackend<Bls12_377Field, Marlin>>::setup(srs, program.clone()).unwrap();
        let interpreter = Interpreter::default();

        let witness = interpreter
            .execute(&program, &[Bls12_377Field::from(42)])
            .unwrap();

        let proof =
            <Ark as Backend<Bls12_377Field, Marlin>>::generate_proof(program, witness, keypair.pk);
        let ans = <Ark as Backend<Bls12_377Field, Marlin>>::verify(keypair.vk, proof);

        assert!(ans);
    }

    #[test]
    fn verify_bw6_761_field() {
        let program: Prog<Bw6_761Field> = Prog {
            main: Function {
                id: String::from("main"),
                arguments: vec![FlatVariable::new(0)],
                returns: vec![FlatVariable::public(0)],
                statements: vec![
                    Statement::Constraint(
                        QuadComb::from_linear_combinations(
                            FlatVariable::new(0).into(),
                            FlatVariable::new(0).into(),
                        ),
                        FlatVariable::new(1).into(),
                    ),
                    Statement::Constraint(
                        FlatVariable::new(1).into(),
                        FlatVariable::public(0).into(),
                    ),
                ],
            },
            private: vec![true],
        };

        let srs = <Ark as UniversalBackend<Bw6_761Field, Marlin>>::universal_setup(5);
        let keypair =
            <Ark as UniversalBackend<Bw6_761Field, Marlin>>::setup(srs, program.clone()).unwrap();
        let interpreter = Interpreter::default();

        let witness = interpreter
            .execute(&program, &[Bw6_761Field::from(42)])
            .unwrap();

        let proof =
            <Ark as Backend<Bw6_761Field, Marlin>>::generate_proof(program, witness, keypair.pk);
        let ans = <Ark as Backend<Bw6_761Field, Marlin>>::verify(keypair.vk, proof);

        assert!(ans);
    }
}
