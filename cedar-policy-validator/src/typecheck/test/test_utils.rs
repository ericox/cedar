/*
 * Copyright Cedar Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Contains utility functions for testing that expression typecheck or fail to
//! typecheck correctly.
// GRCOV_STOP_COVERAGE

use cool_asserts::assert_matches;
use std::{collections::HashSet, sync::Arc};

use cedar_policy_core::ast::{EntityUID, Expr, PolicyID, Template, ACTION_ENTITY_TYPE};
use cedar_policy_core::extensions::Extensions;
use cedar_policy_core::parser::Loc;

use crate::{
    json_schema,
    typecheck::{TypecheckAnswer, Typechecker},
    types::{CapabilitySet, OpenTag, RequestEnv, Type},
    validation_errors::UnexpectedTypeHelp,
    NamespaceDefinitionWithActionAttributes, RawName, ValidationError, ValidationMode,
    ValidationWarning, ValidatorSchema,
};

use similar_asserts::assert_eq;

// Placeholder policy id for use when typechecking an expression directly.
pub fn expr_id_placeholder() -> PolicyID {
    PolicyID::from_string("expr")
}

/// Get `Loc` corresponding to `snippet` in `src`. Returns an option because we
/// always want an `Option<Loc>` instead of a `Loc`. Panics if `snippet` is not
/// in `src` to fail fast in tests.
#[track_caller]
pub fn get_loc(src: impl AsRef<str>, snippet: impl AsRef<str>) -> Option<Loc> {
    let start = src
        .as_ref()
        .find(snippet.as_ref())
        .expect("Snippet does not exist in source!");
    let end = start + snippet.as_ref().len();
    Some(Loc::new(start..end, src.as_ref().into()))
}

impl ValidationError {
    /// Testing utility for an unexpected type error when exactly one type was
    /// expected.
    pub(crate) fn expected_type(
        source_loc: Option<Loc>,
        policy_id: PolicyID,
        expected: Type,
        actual: Type,
        help: Option<UnexpectedTypeHelp>,
    ) -> Self {
        ValidationError::expected_one_of_types(source_loc, policy_id, vec![expected], actual, help)
    }
}

impl Type {
    /// Construct a named entity reference type using the `Name` resulting from
    /// parsing the `name` string. This function will panic on a parse error.
    pub(crate) fn named_entity_reference_from_str(name: &str) -> Type {
        Type::named_entity_reference(name.parse().unwrap_or_else(|_| {
            panic!("Expected that {} would be a valid entity type name.", name)
        }))
    }
}

impl Typechecker<'_> {
    /// Typecheck an expression outside the context of a policy. This is
    /// currently only used for testing.
    pub(crate) fn typecheck_expr<'a>(
        &self,
        e: &'a Expr,
        unique_type_errors: &mut HashSet<ValidationError>,
    ) -> TypecheckAnswer<'a> {
        // Using bogus entity type names here for testing. They'll be treated as
        // having empty attribute records, so tests will behave as expected.
        let request_env = RequestEnv::DeclaredAction {
            principal: &"Principal"
                .parse()
                .expect("Placeholder type \"Principal\" failed to parse as valid type name."),
            action: &EntityUID::with_eid_and_type(ACTION_ENTITY_TYPE, "action")
                .expect("ACTION_ENTITY_TYPE failed to parse as type name."),
            resource: &"Resource"
                .parse()
                .expect("Placeholder type \"Resource\" failed to parse as valid type name."),
            context: &Type::record_with_attributes(None, OpenTag::ClosedAttributes),
            principal_slot: None,
            resource_slot: None,
        };
        let mut type_errors = Vec::new();
        let ans = self.typecheck(&request_env, &CapabilitySet::new(), e, &mut type_errors);
        unique_type_errors.extend(type_errors);
        ans
    }
}

/// Assert expected == actual by by asserting expected <: actual && actual <: expected.
/// In the future it might better to only assert actual <: expected to allow
/// improvement to the typechecker to return more specific types.
#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_types_eq(schema: &ValidatorSchema, expected: &Type, actual: &Type) {
    assert!(
            Type::is_subtype(schema, expected, actual, ValidationMode::Permissive),
            "Type equality assertion failed: the expected type is not a subtype of the actual type.\nexpected: {:#?}\nactual: {:#?}", expected, actual);
    assert!(
            Type::is_subtype(schema, actual, expected, ValidationMode::Permissive),
             "Type equality assertion failed: the actual type is not a subtype of the expected type.\nexpected: {:#?}\nactual: {:#?}", expected, actual);
}

/// Assert that every [`ValidationError`] in the expected list of type errors appears
/// in the expected list of type errors, and that the expected number of
/// type errors were generated.
#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_expected_type_errors(
    expected: impl IntoIterator<Item = ValidationError>,
    actual: &HashSet<ValidationError>,
) {
    assert_eq!(&expected.into_iter().collect::<HashSet<_>>(), actual)
}

/// Assert that every `ValidationWarning` in the expected list of warnings
/// appears in the expected list of warnings, and that the expected number of
/// warnings were generated.
#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_expected_warnings(
    expected: impl IntoIterator<Item = ValidationWarning>,
    actual: &HashSet<ValidationWarning>,
) {
    assert_eq!(&expected.into_iter().collect::<HashSet<_>>(), actual,)
}

/// Unifies a bunch of different ways we specify schemas in tests
pub(crate) trait SchemaProvider {
    /// Produce the schema, panicking (with a nice error message as appropriate) if it is not a valid schema.
    fn schema(self) -> ValidatorSchema;
}

impl SchemaProvider for ValidatorSchema {
    fn schema(self) -> ValidatorSchema {
        self
    }
}

impl SchemaProvider for json_schema::Fragment<RawName> {
    fn schema(self) -> ValidatorSchema {
        self.try_into()
            .unwrap_or_else(|e| panic!("failed to construct schema: {:?}", miette::Report::new(e)))
    }
}

impl SchemaProvider for json_schema::NamespaceDefinition<RawName> {
    fn schema(self) -> ValidatorSchema {
        self.try_into()
            .unwrap_or_else(|e| panic!("failed to construct schema: {:?}", miette::Report::new(e)))
    }
}

impl SchemaProvider for NamespaceDefinitionWithActionAttributes<RawName> {
    fn schema(self) -> ValidatorSchema {
        self.try_into()
            .unwrap_or_else(|e| panic!("failed to construct schema: {:?}", miette::Report::new(e)))
    }
}

impl<'a> SchemaProvider for &'a str {
    fn schema(self) -> ValidatorSchema {
        ValidatorSchema::from_cedarschema_str(self, Extensions::all_available())
            .unwrap_or_else(|e| panic!("failed to construct schema: {:?}", miette::Report::new(e)))
            .0
    }
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_policy_typechecks(
    schema: impl SchemaProvider,
    policy: impl Into<Arc<Template>>,
) {
    assert_policy_typechecks_for_mode(schema, policy, ValidationMode::Strict)
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_policy_typechecks_for_mode(
    schema: impl SchemaProvider,
    policy: impl Into<Arc<Template>>,
    mode: ValidationMode,
) {
    let policy = policy.into();
    let schema = schema.schema();
    let mut typechecker = Typechecker::new(&schema, mode, expr_id_placeholder());
    let mut type_errors: HashSet<ValidationError> = HashSet::new();
    let mut warnings: HashSet<ValidationWarning> = HashSet::new();
    let typechecked = typechecker.typecheck_policy(&policy, &mut type_errors, &mut warnings);
    assert_eq!(type_errors, HashSet::new(), "Did not expect any errors.");
    assert!(typechecked, "Expected that policy would typecheck.");

    // Ensure that partial schema validation doesn't cause any policy that
    // should validate with a complete schema to no longer validate with the
    // same complete schema.
    typechecker.mode = ValidationMode::Permissive;
    let typechecked = typechecker.typecheck_policy(&policy, &mut type_errors, &mut warnings);
    assert_eq!(
        type_errors,
        HashSet::new(),
        "Did not expect any errors under partial schema validation."
    );
    assert!(
        typechecked,
        "Expected that policy would typecheck under partial schema validation."
    );
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_policy_typecheck_fails(
    schema: impl SchemaProvider,
    policy: impl Into<Arc<Template>>,
    expected_type_errors: impl IntoIterator<Item = ValidationError>,
) {
    assert_policy_typecheck_fails_for_mode(
        schema,
        policy,
        expected_type_errors,
        ValidationMode::Strict,
    )
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_policy_typecheck_warns(
    schema: impl SchemaProvider,
    policy: impl Into<Arc<Template>>,
    expected_warnings: impl IntoIterator<Item = ValidationWarning>,
) {
    assert_policy_typecheck_warns_for_mode(
        schema,
        policy,
        expected_warnings,
        ValidationMode::Strict,
    )
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_policy_typecheck_fails_for_mode(
    schema: impl SchemaProvider,
    policy: impl Into<Arc<Template>>,
    expected_type_errors: impl IntoIterator<Item = ValidationError>,
    mode: ValidationMode,
) {
    let policy = policy.into();
    let schema = schema.schema();
    let typechecker = Typechecker::new(&schema, mode, policy.id().clone());
    let mut type_errors: HashSet<ValidationError> = HashSet::new();
    let mut warnings: HashSet<ValidationWarning> = HashSet::new();
    let typechecked = typechecker.typecheck_policy(&policy, &mut type_errors, &mut warnings);
    assert_expected_type_errors(expected_type_errors, &type_errors);
    assert!(!typechecked, "Expected that policy would not typecheck.");
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_policy_typecheck_warns_for_mode(
    schema: impl SchemaProvider,
    policy: impl Into<Arc<Template>>,
    expected_warnings: impl IntoIterator<Item = ValidationWarning>,
    mode: ValidationMode,
) {
    let policy = policy.into();
    let schema = schema.schema();
    let typechecker = Typechecker::new(&schema, mode, policy.id().clone());
    let mut type_errors: HashSet<ValidationError> = HashSet::new();
    let mut warnings: HashSet<ValidationWarning> = HashSet::new();
    let typechecked = typechecker.typecheck_policy(&policy, &mut type_errors, &mut warnings);
    assert_expected_warnings(expected_warnings, &warnings);
    assert!(
        typechecked,
        "Expected that policy would typecheck (with warnings)."
    );
}

/// Assert that expr type checks successfully with a particular type, and
/// that it does not generate any type errors.
#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_typechecks(schema: impl SchemaProvider, expr: Expr, expected: Type) {
    assert_typechecks_for_mode(schema, expr, expected, ValidationMode::Strict);
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_typechecks_for_mode(
    schema: impl SchemaProvider,
    expr: Expr,
    expected: Type,
    mode: ValidationMode,
) {
    let schema = schema.schema();
    let typechecker = Typechecker::new(&schema, mode, expr_id_placeholder());
    let mut type_errors = HashSet::new();
    let actual = typechecker.typecheck_expr(&expr, &mut type_errors);
    assert_matches!(actual, TypecheckAnswer::TypecheckSuccess { expr_type, .. } => {
        assert_types_eq(typechecker.schema, &expected, &expr_type.into_data().expect("Typechecked expression must have type"));
    });
    assert_eq!(
        type_errors,
        HashSet::new(),
        "Did not expect any errors, saw {:#?}.",
        type_errors
    );
}

/// Assert that typechecking fails, generating some `ValidationErrors` for the
/// expressions. Failed type checking will still return a type that is used
/// to continue typechecking, so the `expected` type must match the returned
/// type for this to pass.
#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_typecheck_fails(
    schema: impl SchemaProvider,
    expr: Expr,
    expected_ty: Option<Type>,
    expected_type_errors: impl IntoIterator<Item = ValidationError>,
) {
    assert_typecheck_fails_for_mode(
        schema,
        expr,
        expected_ty,
        expected_type_errors,
        ValidationMode::Strict,
    )
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_typecheck_fails_for_mode(
    schema: impl SchemaProvider,
    expr: Expr,
    expected_ty: Option<Type>,
    expected_type_errors: impl IntoIterator<Item = ValidationError>,
    mode: ValidationMode,
) {
    let schema = schema.schema();
    let typechecker = Typechecker::new(&schema, mode, expr_id_placeholder());
    let mut type_errors = HashSet::new();
    let actual = typechecker.typecheck_expr(&expr, &mut type_errors);
    assert_matches!(actual, TypecheckAnswer::TypecheckFail { expr_recovery_type } => {
        match (expected_ty.as_ref(), expr_recovery_type.data()) {
            (None, None) => (),
            (Some(expected_ty), Some(actual_ty)) => {
                assert_types_eq(typechecker.schema, expected_ty, actual_ty);
            }
            _ => panic!("Expected that actual type would be defined iff expected type is defined."),
        }
        assert_expected_type_errors(expected_type_errors, &type_errors);
    });
}

pub(crate) fn empty_schema_file() -> json_schema::NamespaceDefinition<RawName> {
    json_schema::NamespaceDefinition::new([], [])
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_typechecks_empty_schema(expr: Expr, expected: Type) {
    assert_typechecks(empty_schema_file(), expr, expected)
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_typechecks_empty_schema_permissive(expr: Expr, expected: Type) {
    assert_typechecks_for_mode(
        empty_schema_file(),
        expr,
        expected,
        ValidationMode::Permissive,
    )
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_typecheck_fails_empty_schema(
    expr: Expr,
    expected: Type,
    type_errors: impl IntoIterator<Item = ValidationError>,
) {
    assert_typecheck_fails(empty_schema_file(), expr, Some(expected), type_errors);
}

#[track_caller] // report the caller's location as the location of the panic, not the location in this function
pub(crate) fn assert_typecheck_fails_empty_schema_without_type(
    expr: Expr,
    type_errors: impl IntoIterator<Item = ValidationError>,
) {
    assert_typecheck_fails(empty_schema_file(), expr, None, type_errors);
}
