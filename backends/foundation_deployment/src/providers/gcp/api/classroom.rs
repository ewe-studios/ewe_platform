//! ClassroomProvider - State-aware classroom API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       classroom API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::classroom::{
    classroom_courses_create_builder, classroom_courses_create_task,
    classroom_courses_delete_builder, classroom_courses_delete_task,
    classroom_courses_get_builder, classroom_courses_get_task,
    classroom_courses_get_grading_period_settings_builder, classroom_courses_get_grading_period_settings_task,
    classroom_courses_list_builder, classroom_courses_list_task,
    classroom_courses_patch_builder, classroom_courses_patch_task,
    classroom_courses_update_builder, classroom_courses_update_task,
    classroom_courses_update_grading_period_settings_builder, classroom_courses_update_grading_period_settings_task,
    classroom_courses_aliases_create_builder, classroom_courses_aliases_create_task,
    classroom_courses_aliases_delete_builder, classroom_courses_aliases_delete_task,
    classroom_courses_aliases_list_builder, classroom_courses_aliases_list_task,
    classroom_courses_announcements_create_builder, classroom_courses_announcements_create_task,
    classroom_courses_announcements_delete_builder, classroom_courses_announcements_delete_task,
    classroom_courses_announcements_get_builder, classroom_courses_announcements_get_task,
    classroom_courses_announcements_get_add_on_context_builder, classroom_courses_announcements_get_add_on_context_task,
    classroom_courses_announcements_list_builder, classroom_courses_announcements_list_task,
    classroom_courses_announcements_modify_assignees_builder, classroom_courses_announcements_modify_assignees_task,
    classroom_courses_announcements_patch_builder, classroom_courses_announcements_patch_task,
    classroom_courses_announcements_add_on_attachments_create_builder, classroom_courses_announcements_add_on_attachments_create_task,
    classroom_courses_announcements_add_on_attachments_delete_builder, classroom_courses_announcements_add_on_attachments_delete_task,
    classroom_courses_announcements_add_on_attachments_get_builder, classroom_courses_announcements_add_on_attachments_get_task,
    classroom_courses_announcements_add_on_attachments_list_builder, classroom_courses_announcements_add_on_attachments_list_task,
    classroom_courses_announcements_add_on_attachments_patch_builder, classroom_courses_announcements_add_on_attachments_patch_task,
    classroom_courses_course_work_create_builder, classroom_courses_course_work_create_task,
    classroom_courses_course_work_delete_builder, classroom_courses_course_work_delete_task,
    classroom_courses_course_work_get_builder, classroom_courses_course_work_get_task,
    classroom_courses_course_work_get_add_on_context_builder, classroom_courses_course_work_get_add_on_context_task,
    classroom_courses_course_work_list_builder, classroom_courses_course_work_list_task,
    classroom_courses_course_work_modify_assignees_builder, classroom_courses_course_work_modify_assignees_task,
    classroom_courses_course_work_patch_builder, classroom_courses_course_work_patch_task,
    classroom_courses_course_work_update_rubric_builder, classroom_courses_course_work_update_rubric_task,
    classroom_courses_course_work_add_on_attachments_create_builder, classroom_courses_course_work_add_on_attachments_create_task,
    classroom_courses_course_work_add_on_attachments_delete_builder, classroom_courses_course_work_add_on_attachments_delete_task,
    classroom_courses_course_work_add_on_attachments_get_builder, classroom_courses_course_work_add_on_attachments_get_task,
    classroom_courses_course_work_add_on_attachments_list_builder, classroom_courses_course_work_add_on_attachments_list_task,
    classroom_courses_course_work_add_on_attachments_patch_builder, classroom_courses_course_work_add_on_attachments_patch_task,
    classroom_courses_course_work_add_on_attachments_student_submissions_get_builder, classroom_courses_course_work_add_on_attachments_student_submissions_get_task,
    classroom_courses_course_work_add_on_attachments_student_submissions_patch_builder, classroom_courses_course_work_add_on_attachments_student_submissions_patch_task,
    classroom_courses_course_work_rubrics_create_builder, classroom_courses_course_work_rubrics_create_task,
    classroom_courses_course_work_rubrics_delete_builder, classroom_courses_course_work_rubrics_delete_task,
    classroom_courses_course_work_rubrics_get_builder, classroom_courses_course_work_rubrics_get_task,
    classroom_courses_course_work_rubrics_list_builder, classroom_courses_course_work_rubrics_list_task,
    classroom_courses_course_work_rubrics_patch_builder, classroom_courses_course_work_rubrics_patch_task,
    classroom_courses_course_work_student_submissions_get_builder, classroom_courses_course_work_student_submissions_get_task,
    classroom_courses_course_work_student_submissions_list_builder, classroom_courses_course_work_student_submissions_list_task,
    classroom_courses_course_work_student_submissions_modify_attachments_builder, classroom_courses_course_work_student_submissions_modify_attachments_task,
    classroom_courses_course_work_student_submissions_patch_builder, classroom_courses_course_work_student_submissions_patch_task,
    classroom_courses_course_work_student_submissions_reclaim_builder, classroom_courses_course_work_student_submissions_reclaim_task,
    classroom_courses_course_work_student_submissions_return_builder, classroom_courses_course_work_student_submissions_return_task,
    classroom_courses_course_work_student_submissions_turn_in_builder, classroom_courses_course_work_student_submissions_turn_in_task,
    classroom_courses_course_work_materials_create_builder, classroom_courses_course_work_materials_create_task,
    classroom_courses_course_work_materials_delete_builder, classroom_courses_course_work_materials_delete_task,
    classroom_courses_course_work_materials_get_builder, classroom_courses_course_work_materials_get_task,
    classroom_courses_course_work_materials_get_add_on_context_builder, classroom_courses_course_work_materials_get_add_on_context_task,
    classroom_courses_course_work_materials_list_builder, classroom_courses_course_work_materials_list_task,
    classroom_courses_course_work_materials_patch_builder, classroom_courses_course_work_materials_patch_task,
    classroom_courses_course_work_materials_add_on_attachments_create_builder, classroom_courses_course_work_materials_add_on_attachments_create_task,
    classroom_courses_course_work_materials_add_on_attachments_delete_builder, classroom_courses_course_work_materials_add_on_attachments_delete_task,
    classroom_courses_course_work_materials_add_on_attachments_get_builder, classroom_courses_course_work_materials_add_on_attachments_get_task,
    classroom_courses_course_work_materials_add_on_attachments_list_builder, classroom_courses_course_work_materials_add_on_attachments_list_task,
    classroom_courses_course_work_materials_add_on_attachments_patch_builder, classroom_courses_course_work_materials_add_on_attachments_patch_task,
    classroom_courses_posts_get_add_on_context_builder, classroom_courses_posts_get_add_on_context_task,
    classroom_courses_posts_add_on_attachments_create_builder, classroom_courses_posts_add_on_attachments_create_task,
    classroom_courses_posts_add_on_attachments_delete_builder, classroom_courses_posts_add_on_attachments_delete_task,
    classroom_courses_posts_add_on_attachments_get_builder, classroom_courses_posts_add_on_attachments_get_task,
    classroom_courses_posts_add_on_attachments_list_builder, classroom_courses_posts_add_on_attachments_list_task,
    classroom_courses_posts_add_on_attachments_patch_builder, classroom_courses_posts_add_on_attachments_patch_task,
    classroom_courses_posts_add_on_attachments_student_submissions_get_builder, classroom_courses_posts_add_on_attachments_student_submissions_get_task,
    classroom_courses_posts_add_on_attachments_student_submissions_patch_builder, classroom_courses_posts_add_on_attachments_student_submissions_patch_task,
    classroom_courses_student_groups_create_builder, classroom_courses_student_groups_create_task,
    classroom_courses_student_groups_delete_builder, classroom_courses_student_groups_delete_task,
    classroom_courses_student_groups_list_builder, classroom_courses_student_groups_list_task,
    classroom_courses_student_groups_patch_builder, classroom_courses_student_groups_patch_task,
    classroom_courses_student_groups_student_group_members_create_builder, classroom_courses_student_groups_student_group_members_create_task,
    classroom_courses_student_groups_student_group_members_delete_builder, classroom_courses_student_groups_student_group_members_delete_task,
    classroom_courses_student_groups_student_group_members_list_builder, classroom_courses_student_groups_student_group_members_list_task,
    classroom_courses_students_create_builder, classroom_courses_students_create_task,
    classroom_courses_students_delete_builder, classroom_courses_students_delete_task,
    classroom_courses_students_get_builder, classroom_courses_students_get_task,
    classroom_courses_students_list_builder, classroom_courses_students_list_task,
    classroom_courses_teachers_create_builder, classroom_courses_teachers_create_task,
    classroom_courses_teachers_delete_builder, classroom_courses_teachers_delete_task,
    classroom_courses_teachers_get_builder, classroom_courses_teachers_get_task,
    classroom_courses_teachers_list_builder, classroom_courses_teachers_list_task,
    classroom_courses_topics_create_builder, classroom_courses_topics_create_task,
    classroom_courses_topics_delete_builder, classroom_courses_topics_delete_task,
    classroom_courses_topics_get_builder, classroom_courses_topics_get_task,
    classroom_courses_topics_list_builder, classroom_courses_topics_list_task,
    classroom_courses_topics_patch_builder, classroom_courses_topics_patch_task,
    classroom_invitations_accept_builder, classroom_invitations_accept_task,
    classroom_invitations_create_builder, classroom_invitations_create_task,
    classroom_invitations_delete_builder, classroom_invitations_delete_task,
    classroom_invitations_get_builder, classroom_invitations_get_task,
    classroom_invitations_list_builder, classroom_invitations_list_task,
    classroom_registrations_create_builder, classroom_registrations_create_task,
    classroom_registrations_delete_builder, classroom_registrations_delete_task,
    classroom_user_profiles_get_builder, classroom_user_profiles_get_task,
    classroom_user_profiles_guardian_invitations_create_builder, classroom_user_profiles_guardian_invitations_create_task,
    classroom_user_profiles_guardian_invitations_get_builder, classroom_user_profiles_guardian_invitations_get_task,
    classroom_user_profiles_guardian_invitations_list_builder, classroom_user_profiles_guardian_invitations_list_task,
    classroom_user_profiles_guardian_invitations_patch_builder, classroom_user_profiles_guardian_invitations_patch_task,
    classroom_user_profiles_guardians_delete_builder, classroom_user_profiles_guardians_delete_task,
    classroom_user_profiles_guardians_get_builder, classroom_user_profiles_guardians_get_task,
    classroom_user_profiles_guardians_list_builder, classroom_user_profiles_guardians_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::classroom::AddOnAttachment;
use crate::providers::gcp::clients::classroom::AddOnAttachmentStudentSubmission;
use crate::providers::gcp::clients::classroom::AddOnContext;
use crate::providers::gcp::clients::classroom::Announcement;
use crate::providers::gcp::clients::classroom::Course;
use crate::providers::gcp::clients::classroom::CourseAlias;
use crate::providers::gcp::clients::classroom::CourseWork;
use crate::providers::gcp::clients::classroom::CourseWorkMaterial;
use crate::providers::gcp::clients::classroom::Empty;
use crate::providers::gcp::clients::classroom::GradingPeriodSettings;
use crate::providers::gcp::clients::classroom::Guardian;
use crate::providers::gcp::clients::classroom::GuardianInvitation;
use crate::providers::gcp::clients::classroom::Invitation;
use crate::providers::gcp::clients::classroom::ListAddOnAttachmentsResponse;
use crate::providers::gcp::clients::classroom::ListAnnouncementsResponse;
use crate::providers::gcp::clients::classroom::ListCourseAliasesResponse;
use crate::providers::gcp::clients::classroom::ListCourseWorkMaterialResponse;
use crate::providers::gcp::clients::classroom::ListCourseWorkResponse;
use crate::providers::gcp::clients::classroom::ListCoursesResponse;
use crate::providers::gcp::clients::classroom::ListGuardianInvitationsResponse;
use crate::providers::gcp::clients::classroom::ListGuardiansResponse;
use crate::providers::gcp::clients::classroom::ListInvitationsResponse;
use crate::providers::gcp::clients::classroom::ListRubricsResponse;
use crate::providers::gcp::clients::classroom::ListStudentGroupMembersResponse;
use crate::providers::gcp::clients::classroom::ListStudentGroupsResponse;
use crate::providers::gcp::clients::classroom::ListStudentSubmissionsResponse;
use crate::providers::gcp::clients::classroom::ListStudentsResponse;
use crate::providers::gcp::clients::classroom::ListTeachersResponse;
use crate::providers::gcp::clients::classroom::ListTopicResponse;
use crate::providers::gcp::clients::classroom::Registration;
use crate::providers::gcp::clients::classroom::Rubric;
use crate::providers::gcp::clients::classroom::Student;
use crate::providers::gcp::clients::classroom::StudentGroup;
use crate::providers::gcp::clients::classroom::StudentGroupMember;
use crate::providers::gcp::clients::classroom::StudentSubmission;
use crate::providers::gcp::clients::classroom::Teacher;
use crate::providers::gcp::clients::classroom::Topic;
use crate::providers::gcp::clients::classroom::UserProfile;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAliasesCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAliasesDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAliasesListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsAddOnAttachmentsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsAddOnAttachmentsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsAddOnAttachmentsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsAddOnAttachmentsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsAddOnAttachmentsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsGetAddOnContextArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsModifyAssigneesArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesAnnouncementsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkAddOnAttachmentsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkAddOnAttachmentsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkAddOnAttachmentsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkAddOnAttachmentsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkAddOnAttachmentsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkAddOnAttachmentsStudentSubmissionsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkAddOnAttachmentsStudentSubmissionsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkGetAddOnContextArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsGetAddOnContextArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkMaterialsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkModifyAssigneesArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkRubricsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkRubricsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkRubricsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkRubricsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkRubricsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkStudentSubmissionsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkStudentSubmissionsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkStudentSubmissionsModifyAttachmentsArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkStudentSubmissionsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkStudentSubmissionsReclaimArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkStudentSubmissionsReturnArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkStudentSubmissionsTurnInArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesCourseWorkUpdateRubricArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesGetGradingPeriodSettingsArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesPostsAddOnAttachmentsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesPostsAddOnAttachmentsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesPostsAddOnAttachmentsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesPostsAddOnAttachmentsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesPostsAddOnAttachmentsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesPostsAddOnAttachmentsStudentSubmissionsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesPostsAddOnAttachmentsStudentSubmissionsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesPostsGetAddOnContextArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentGroupsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentGroupsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentGroupsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentGroupsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentGroupsStudentGroupMembersCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentGroupsStudentGroupMembersDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentGroupsStudentGroupMembersListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesStudentsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesTeachersCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesTeachersDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesTeachersGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesTeachersListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesTopicsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesTopicsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesTopicsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesTopicsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesTopicsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesUpdateArgs;
use crate::providers::gcp::clients::classroom::ClassroomCoursesUpdateGradingPeriodSettingsArgs;
use crate::providers::gcp::clients::classroom::ClassroomInvitationsAcceptArgs;
use crate::providers::gcp::clients::classroom::ClassroomInvitationsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomInvitationsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomInvitationsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomRegistrationsDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomUserProfilesGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomUserProfilesGuardianInvitationsCreateArgs;
use crate::providers::gcp::clients::classroom::ClassroomUserProfilesGuardianInvitationsGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomUserProfilesGuardianInvitationsListArgs;
use crate::providers::gcp::clients::classroom::ClassroomUserProfilesGuardianInvitationsPatchArgs;
use crate::providers::gcp::clients::classroom::ClassroomUserProfilesGuardiansDeleteArgs;
use crate::providers::gcp::clients::classroom::ClassroomUserProfilesGuardiansGetArgs;
use crate::providers::gcp::clients::classroom::ClassroomUserProfilesGuardiansListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ClassroomProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
/// * `R` - DNS resolver type for HTTP client
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let http_client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
/// let client = ProviderClient::new("my-project", "dev", state_store, http_client);
/// let provider = ClassroomProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ClassroomProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ClassroomProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ClassroomProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ClassroomProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Classroom courses create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Course result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_create(
        &self,
        args: &ClassroomCoursesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Course, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_delete(
        &self,
        args: &ClassroomCoursesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_delete_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Course result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_get(
        &self,
        args: &ClassroomCoursesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Course, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_get_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses get grading period settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GradingPeriodSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_get_grading_period_settings(
        &self,
        args: &ClassroomCoursesGetGradingPeriodSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GradingPeriodSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_get_grading_period_settings_builder(
            &self.http_client,
            &args.courseId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_get_grading_period_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCoursesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_list(
        &self,
        args: &ClassroomCoursesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCoursesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_list_builder(
            &self.http_client,
            &args.courseStates,
            &args.pageSize,
            &args.pageToken,
            &args.studentId,
            &args.teacherId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Course result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_patch(
        &self,
        args: &ClassroomCoursesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Course, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_patch_builder(
            &self.http_client,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Course result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_update(
        &self,
        args: &ClassroomCoursesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Course, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_update_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses update grading period settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GradingPeriodSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_update_grading_period_settings(
        &self,
        args: &ClassroomCoursesUpdateGradingPeriodSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GradingPeriodSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_update_grading_period_settings_builder(
            &self.http_client,
            &args.courseId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_update_grading_period_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses aliases create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CourseAlias result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_aliases_create(
        &self,
        args: &ClassroomCoursesAliasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CourseAlias, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_aliases_create_builder(
            &self.http_client,
            &args.courseId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_aliases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses aliases delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_aliases_delete(
        &self,
        args: &ClassroomCoursesAliasesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_aliases_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.alias,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_aliases_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses aliases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCourseAliasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_aliases_list(
        &self,
        args: &ClassroomCoursesAliasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCourseAliasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_aliases_list_builder(
            &self.http_client,
            &args.courseId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_aliases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Announcement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_create(
        &self,
        args: &ClassroomCoursesAnnouncementsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Announcement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_create_builder(
            &self.http_client,
            &args.courseId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_delete(
        &self,
        args: &ClassroomCoursesAnnouncementsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Announcement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_announcements_get(
        &self,
        args: &ClassroomCoursesAnnouncementsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Announcement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_get_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements get add on context.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnContext result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_get_add_on_context(
        &self,
        args: &ClassroomCoursesAnnouncementsGetAddOnContextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnContext, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_get_add_on_context_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.addOnToken,
            &args.attachmentId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_get_add_on_context_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAnnouncementsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_announcements_list(
        &self,
        args: &ClassroomCoursesAnnouncementsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAnnouncementsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_list_builder(
            &self.http_client,
            &args.courseId,
            &args.announcementStates,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements modify assignees.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Announcement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_modify_assignees(
        &self,
        args: &ClassroomCoursesAnnouncementsModifyAssigneesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Announcement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_modify_assignees_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_modify_assignees_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Announcement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_patch(
        &self,
        args: &ClassroomCoursesAnnouncementsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Announcement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements add on attachments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_add_on_attachments_create(
        &self,
        args: &ClassroomCoursesAnnouncementsAddOnAttachmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_add_on_attachments_create_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.addOnToken,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_add_on_attachments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements add on attachments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_add_on_attachments_delete(
        &self,
        args: &ClassroomCoursesAnnouncementsAddOnAttachmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_add_on_attachments_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_add_on_attachments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements add on attachments get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_add_on_attachments_get(
        &self,
        args: &ClassroomCoursesAnnouncementsAddOnAttachmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_add_on_attachments_get_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_add_on_attachments_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements add on attachments list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAddOnAttachmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_add_on_attachments_list(
        &self,
        args: &ClassroomCoursesAnnouncementsAddOnAttachmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAddOnAttachmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_add_on_attachments_list_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.pageSize,
            &args.pageToken,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_add_on_attachments_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses announcements add on attachments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_announcements_add_on_attachments_patch(
        &self,
        args: &ClassroomCoursesAnnouncementsAddOnAttachmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_announcements_add_on_attachments_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.postId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_announcements_add_on_attachments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CourseWork result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_create(
        &self,
        args: &ClassroomCoursesCourseWorkCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CourseWork, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_create_builder(
            &self.http_client,
            &args.courseId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_delete(
        &self,
        args: &ClassroomCoursesCourseWorkDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CourseWork result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_course_work_get(
        &self,
        args: &ClassroomCoursesCourseWorkGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CourseWork, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_get_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work get add on context.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnContext result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_get_add_on_context(
        &self,
        args: &ClassroomCoursesCourseWorkGetAddOnContextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnContext, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_get_add_on_context_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.addOnToken,
            &args.attachmentId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_get_add_on_context_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCourseWorkResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_course_work_list(
        &self,
        args: &ClassroomCoursesCourseWorkListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCourseWorkResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_list_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkStates,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work modify assignees.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CourseWork result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_modify_assignees(
        &self,
        args: &ClassroomCoursesCourseWorkModifyAssigneesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CourseWork, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_modify_assignees_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_modify_assignees_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CourseWork result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_patch(
        &self,
        args: &ClassroomCoursesCourseWorkPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CourseWork, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work update rubric.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Rubric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_update_rubric(
        &self,
        args: &ClassroomCoursesCourseWorkUpdateRubricArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Rubric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_update_rubric_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_update_rubric_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work add on attachments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_add_on_attachments_create(
        &self,
        args: &ClassroomCoursesCourseWorkAddOnAttachmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_add_on_attachments_create_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.addOnToken,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_add_on_attachments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work add on attachments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_add_on_attachments_delete(
        &self,
        args: &ClassroomCoursesCourseWorkAddOnAttachmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_add_on_attachments_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_add_on_attachments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work add on attachments get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_add_on_attachments_get(
        &self,
        args: &ClassroomCoursesCourseWorkAddOnAttachmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_add_on_attachments_get_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_add_on_attachments_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work add on attachments list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAddOnAttachmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_add_on_attachments_list(
        &self,
        args: &ClassroomCoursesCourseWorkAddOnAttachmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAddOnAttachmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_add_on_attachments_list_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.pageSize,
            &args.pageToken,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_add_on_attachments_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work add on attachments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_add_on_attachments_patch(
        &self,
        args: &ClassroomCoursesCourseWorkAddOnAttachmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_add_on_attachments_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.postId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_add_on_attachments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work add on attachments student submissions get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachmentStudentSubmission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_add_on_attachments_student_submissions_get(
        &self,
        args: &ClassroomCoursesCourseWorkAddOnAttachmentsStudentSubmissionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachmentStudentSubmission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_add_on_attachments_student_submissions_get_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.submissionId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_add_on_attachments_student_submissions_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work add on attachments student submissions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachmentStudentSubmission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_add_on_attachments_student_submissions_patch(
        &self,
        args: &ClassroomCoursesCourseWorkAddOnAttachmentsStudentSubmissionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachmentStudentSubmission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_add_on_attachments_student_submissions_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.submissionId,
            &args.postId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_add_on_attachments_student_submissions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work rubrics create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Rubric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_rubrics_create(
        &self,
        args: &ClassroomCoursesCourseWorkRubricsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Rubric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_rubrics_create_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_rubrics_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work rubrics delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_rubrics_delete(
        &self,
        args: &ClassroomCoursesCourseWorkRubricsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_rubrics_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_rubrics_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work rubrics get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Rubric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_course_work_rubrics_get(
        &self,
        args: &ClassroomCoursesCourseWorkRubricsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Rubric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_rubrics_get_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_rubrics_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work rubrics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRubricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_course_work_rubrics_list(
        &self,
        args: &ClassroomCoursesCourseWorkRubricsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRubricsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_rubrics_list_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_rubrics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work rubrics patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Rubric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_rubrics_patch(
        &self,
        args: &ClassroomCoursesCourseWorkRubricsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Rubric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_rubrics_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_rubrics_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work student submissions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StudentSubmission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_course_work_student_submissions_get(
        &self,
        args: &ClassroomCoursesCourseWorkStudentSubmissionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StudentSubmission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_student_submissions_get_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_student_submissions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work student submissions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStudentSubmissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_course_work_student_submissions_list(
        &self,
        args: &ClassroomCoursesCourseWorkStudentSubmissionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStudentSubmissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_student_submissions_list_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.late,
            &args.pageSize,
            &args.pageToken,
            &args.states,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_student_submissions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work student submissions modify attachments.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StudentSubmission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_student_submissions_modify_attachments(
        &self,
        args: &ClassroomCoursesCourseWorkStudentSubmissionsModifyAttachmentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StudentSubmission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_student_submissions_modify_attachments_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_student_submissions_modify_attachments_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work student submissions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StudentSubmission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_student_submissions_patch(
        &self,
        args: &ClassroomCoursesCourseWorkStudentSubmissionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StudentSubmission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_student_submissions_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_student_submissions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work student submissions reclaim.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_student_submissions_reclaim(
        &self,
        args: &ClassroomCoursesCourseWorkStudentSubmissionsReclaimArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_student_submissions_reclaim_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_student_submissions_reclaim_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work student submissions return.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_student_submissions_return(
        &self,
        args: &ClassroomCoursesCourseWorkStudentSubmissionsReturnArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_student_submissions_return_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_student_submissions_return_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work student submissions turn in.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_student_submissions_turn_in(
        &self,
        args: &ClassroomCoursesCourseWorkStudentSubmissionsTurnInArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_student_submissions_turn_in_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_student_submissions_turn_in_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CourseWorkMaterial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_materials_create(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CourseWorkMaterial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_create_builder(
            &self.http_client,
            &args.courseId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_materials_delete(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CourseWorkMaterial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_course_work_materials_get(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CourseWorkMaterial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_get_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials get add on context.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnContext result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_materials_get_add_on_context(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsGetAddOnContextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnContext, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_get_add_on_context_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.addOnToken,
            &args.attachmentId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_get_add_on_context_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCourseWorkMaterialResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_course_work_materials_list(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCourseWorkMaterialResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_list_builder(
            &self.http_client,
            &args.courseId,
            &args.courseWorkMaterialStates,
            &args.materialDriveId,
            &args.materialLink,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CourseWorkMaterial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_materials_patch(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CourseWorkMaterial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials add on attachments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_materials_add_on_attachments_create(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_add_on_attachments_create_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.addOnToken,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_add_on_attachments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials add on attachments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_materials_add_on_attachments_delete(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_add_on_attachments_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_add_on_attachments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials add on attachments get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_materials_add_on_attachments_get(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_add_on_attachments_get_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_add_on_attachments_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials add on attachments list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAddOnAttachmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_materials_add_on_attachments_list(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAddOnAttachmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_add_on_attachments_list_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.pageSize,
            &args.pageToken,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_add_on_attachments_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses course work materials add on attachments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_course_work_materials_add_on_attachments_patch(
        &self,
        args: &ClassroomCoursesCourseWorkMaterialsAddOnAttachmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_course_work_materials_add_on_attachments_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.itemId,
            &args.attachmentId,
            &args.postId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_course_work_materials_add_on_attachments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses posts get add on context.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnContext result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_posts_get_add_on_context(
        &self,
        args: &ClassroomCoursesPostsGetAddOnContextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnContext, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_posts_get_add_on_context_builder(
            &self.http_client,
            &args.courseId,
            &args.postId,
            &args.addOnToken,
            &args.attachmentId,
            &args.itemId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_posts_get_add_on_context_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses posts add on attachments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_posts_add_on_attachments_create(
        &self,
        args: &ClassroomCoursesPostsAddOnAttachmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_posts_add_on_attachments_create_builder(
            &self.http_client,
            &args.courseId,
            &args.postId,
            &args.addOnToken,
            &args.itemId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_posts_add_on_attachments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses posts add on attachments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_posts_add_on_attachments_delete(
        &self,
        args: &ClassroomCoursesPostsAddOnAttachmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_posts_add_on_attachments_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.postId,
            &args.attachmentId,
            &args.itemId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_posts_add_on_attachments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses posts add on attachments get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_posts_add_on_attachments_get(
        &self,
        args: &ClassroomCoursesPostsAddOnAttachmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_posts_add_on_attachments_get_builder(
            &self.http_client,
            &args.courseId,
            &args.postId,
            &args.attachmentId,
            &args.itemId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_posts_add_on_attachments_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses posts add on attachments list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAddOnAttachmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_posts_add_on_attachments_list(
        &self,
        args: &ClassroomCoursesPostsAddOnAttachmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAddOnAttachmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_posts_add_on_attachments_list_builder(
            &self.http_client,
            &args.courseId,
            &args.postId,
            &args.itemId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_posts_add_on_attachments_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses posts add on attachments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_posts_add_on_attachments_patch(
        &self,
        args: &ClassroomCoursesPostsAddOnAttachmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_posts_add_on_attachments_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.postId,
            &args.attachmentId,
            &args.itemId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_posts_add_on_attachments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses posts add on attachments student submissions get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachmentStudentSubmission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_posts_add_on_attachments_student_submissions_get(
        &self,
        args: &ClassroomCoursesPostsAddOnAttachmentsStudentSubmissionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachmentStudentSubmission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_posts_add_on_attachments_student_submissions_get_builder(
            &self.http_client,
            &args.courseId,
            &args.postId,
            &args.attachmentId,
            &args.submissionId,
            &args.itemId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_posts_add_on_attachments_student_submissions_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses posts add on attachments student submissions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddOnAttachmentStudentSubmission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_posts_add_on_attachments_student_submissions_patch(
        &self,
        args: &ClassroomCoursesPostsAddOnAttachmentsStudentSubmissionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddOnAttachmentStudentSubmission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_posts_add_on_attachments_student_submissions_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.postId,
            &args.attachmentId,
            &args.submissionId,
            &args.itemId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_posts_add_on_attachments_student_submissions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses student groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StudentGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_student_groups_create(
        &self,
        args: &ClassroomCoursesStudentGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StudentGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_student_groups_create_builder(
            &self.http_client,
            &args.courseId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_student_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses student groups delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_student_groups_delete(
        &self,
        args: &ClassroomCoursesStudentGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_student_groups_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_student_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses student groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStudentGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_student_groups_list(
        &self,
        args: &ClassroomCoursesStudentGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStudentGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_student_groups_list_builder(
            &self.http_client,
            &args.courseId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_student_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses student groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StudentGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_student_groups_patch(
        &self,
        args: &ClassroomCoursesStudentGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StudentGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_student_groups_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_student_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses student groups student group members create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StudentGroupMember result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_student_groups_student_group_members_create(
        &self,
        args: &ClassroomCoursesStudentGroupsStudentGroupMembersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StudentGroupMember, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_student_groups_student_group_members_create_builder(
            &self.http_client,
            &args.courseId,
            &args.studentGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_student_groups_student_group_members_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses student groups student group members delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_student_groups_student_group_members_delete(
        &self,
        args: &ClassroomCoursesStudentGroupsStudentGroupMembersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_student_groups_student_group_members_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.studentGroupId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_student_groups_student_group_members_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses student groups student group members list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStudentGroupMembersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_student_groups_student_group_members_list(
        &self,
        args: &ClassroomCoursesStudentGroupsStudentGroupMembersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStudentGroupMembersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_student_groups_student_group_members_list_builder(
            &self.http_client,
            &args.courseId,
            &args.studentGroupId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_student_groups_student_group_members_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses students create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Student result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_students_create(
        &self,
        args: &ClassroomCoursesStudentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Student, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_students_create_builder(
            &self.http_client,
            &args.courseId,
            &args.enrollmentCode,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_students_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses students delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_students_delete(
        &self,
        args: &ClassroomCoursesStudentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_students_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_students_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses students get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Student result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_students_get(
        &self,
        args: &ClassroomCoursesStudentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Student, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_students_get_builder(
            &self.http_client,
            &args.courseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_students_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses students list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStudentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_students_list(
        &self,
        args: &ClassroomCoursesStudentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStudentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_students_list_builder(
            &self.http_client,
            &args.courseId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_students_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses teachers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Teacher result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_teachers_create(
        &self,
        args: &ClassroomCoursesTeachersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Teacher, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_teachers_create_builder(
            &self.http_client,
            &args.courseId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_teachers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses teachers delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_teachers_delete(
        &self,
        args: &ClassroomCoursesTeachersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_teachers_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_teachers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses teachers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Teacher result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_teachers_get(
        &self,
        args: &ClassroomCoursesTeachersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Teacher, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_teachers_get_builder(
            &self.http_client,
            &args.courseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_teachers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses teachers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTeachersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_teachers_list(
        &self,
        args: &ClassroomCoursesTeachersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTeachersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_teachers_list_builder(
            &self.http_client,
            &args.courseId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_teachers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses topics create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_topics_create(
        &self,
        args: &ClassroomCoursesTopicsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_topics_create_builder(
            &self.http_client,
            &args.courseId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_topics_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses topics delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_topics_delete(
        &self,
        args: &ClassroomCoursesTopicsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_topics_delete_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_topics_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses topics get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_topics_get(
        &self,
        args: &ClassroomCoursesTopicsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_topics_get_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_topics_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses topics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTopicResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_courses_topics_list(
        &self,
        args: &ClassroomCoursesTopicsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTopicResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_topics_list_builder(
            &self.http_client,
            &args.courseId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_topics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom courses topics patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_courses_topics_patch(
        &self,
        args: &ClassroomCoursesTopicsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_courses_topics_patch_builder(
            &self.http_client,
            &args.courseId,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_courses_topics_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom invitations accept.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_invitations_accept(
        &self,
        args: &ClassroomInvitationsAcceptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_invitations_accept_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_invitations_accept_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom invitations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invitation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_invitations_create(
        &self,
        args: &ClassroomInvitationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invitation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_invitations_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_invitations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom invitations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_invitations_delete(
        &self,
        args: &ClassroomInvitationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_invitations_delete_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_invitations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom invitations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invitation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_invitations_get(
        &self,
        args: &ClassroomInvitationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invitation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_invitations_get_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_invitations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom invitations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInvitationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_invitations_list(
        &self,
        args: &ClassroomInvitationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInvitationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_invitations_list_builder(
            &self.http_client,
            &args.courseId,
            &args.pageSize,
            &args.pageToken,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_invitations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom registrations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Registration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_registrations_create(
        &self,
        args: &ClassroomRegistrationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Registration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_registrations_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_registrations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom registrations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_registrations_delete(
        &self,
        args: &ClassroomRegistrationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_registrations_delete_builder(
            &self.http_client,
            &args.registrationId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_registrations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom user profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_user_profiles_get(
        &self,
        args: &ClassroomUserProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_user_profiles_get_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_user_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom user profiles guardian invitations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GuardianInvitation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_user_profiles_guardian_invitations_create(
        &self,
        args: &ClassroomUserProfilesGuardianInvitationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GuardianInvitation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_user_profiles_guardian_invitations_create_builder(
            &self.http_client,
            &args.studentId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_user_profiles_guardian_invitations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom user profiles guardian invitations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GuardianInvitation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_user_profiles_guardian_invitations_get(
        &self,
        args: &ClassroomUserProfilesGuardianInvitationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GuardianInvitation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_user_profiles_guardian_invitations_get_builder(
            &self.http_client,
            &args.studentId,
            &args.invitationId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_user_profiles_guardian_invitations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom user profiles guardian invitations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGuardianInvitationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_user_profiles_guardian_invitations_list(
        &self,
        args: &ClassroomUserProfilesGuardianInvitationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGuardianInvitationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_user_profiles_guardian_invitations_list_builder(
            &self.http_client,
            &args.studentId,
            &args.invitedEmailAddress,
            &args.pageSize,
            &args.pageToken,
            &args.states,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_user_profiles_guardian_invitations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom user profiles guardian invitations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GuardianInvitation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_user_profiles_guardian_invitations_patch(
        &self,
        args: &ClassroomUserProfilesGuardianInvitationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GuardianInvitation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_user_profiles_guardian_invitations_patch_builder(
            &self.http_client,
            &args.studentId,
            &args.invitationId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_user_profiles_guardian_invitations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom user profiles guardians delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn classroom_user_profiles_guardians_delete(
        &self,
        args: &ClassroomUserProfilesGuardiansDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_user_profiles_guardians_delete_builder(
            &self.http_client,
            &args.studentId,
            &args.guardianId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_user_profiles_guardians_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom user profiles guardians get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Guardian result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_user_profiles_guardians_get(
        &self,
        args: &ClassroomUserProfilesGuardiansGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Guardian, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_user_profiles_guardians_get_builder(
            &self.http_client,
            &args.studentId,
            &args.guardianId,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_user_profiles_guardians_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Classroom user profiles guardians list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGuardiansResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn classroom_user_profiles_guardians_list(
        &self,
        args: &ClassroomUserProfilesGuardiansListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGuardiansResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = classroom_user_profiles_guardians_list_builder(
            &self.http_client,
            &args.studentId,
            &args.invitedEmailAddress,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = classroom_user_profiles_guardians_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
