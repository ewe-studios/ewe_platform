//! DataprocProvider - State-aware dataproc API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dataproc API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dataproc::{
    dataproc_projects_locations_autoscaling_policies_create_builder, dataproc_projects_locations_autoscaling_policies_create_task,
    dataproc_projects_locations_autoscaling_policies_delete_builder, dataproc_projects_locations_autoscaling_policies_delete_task,
    dataproc_projects_locations_autoscaling_policies_get_builder, dataproc_projects_locations_autoscaling_policies_get_task,
    dataproc_projects_locations_autoscaling_policies_get_iam_policy_builder, dataproc_projects_locations_autoscaling_policies_get_iam_policy_task,
    dataproc_projects_locations_autoscaling_policies_list_builder, dataproc_projects_locations_autoscaling_policies_list_task,
    dataproc_projects_locations_autoscaling_policies_set_iam_policy_builder, dataproc_projects_locations_autoscaling_policies_set_iam_policy_task,
    dataproc_projects_locations_autoscaling_policies_test_iam_permissions_builder, dataproc_projects_locations_autoscaling_policies_test_iam_permissions_task,
    dataproc_projects_locations_autoscaling_policies_update_builder, dataproc_projects_locations_autoscaling_policies_update_task,
    dataproc_projects_locations_batches_analyze_builder, dataproc_projects_locations_batches_analyze_task,
    dataproc_projects_locations_batches_create_builder, dataproc_projects_locations_batches_create_task,
    dataproc_projects_locations_batches_delete_builder, dataproc_projects_locations_batches_delete_task,
    dataproc_projects_locations_batches_get_builder, dataproc_projects_locations_batches_get_task,
    dataproc_projects_locations_batches_list_builder, dataproc_projects_locations_batches_list_task,
    dataproc_projects_locations_batches_spark_applications_access_builder, dataproc_projects_locations_batches_spark_applications_access_task,
    dataproc_projects_locations_batches_spark_applications_access_environment_info_builder, dataproc_projects_locations_batches_spark_applications_access_environment_info_task,
    dataproc_projects_locations_batches_spark_applications_access_job_builder, dataproc_projects_locations_batches_spark_applications_access_job_task,
    dataproc_projects_locations_batches_spark_applications_access_sql_plan_builder, dataproc_projects_locations_batches_spark_applications_access_sql_plan_task,
    dataproc_projects_locations_batches_spark_applications_access_sql_query_builder, dataproc_projects_locations_batches_spark_applications_access_sql_query_task,
    dataproc_projects_locations_batches_spark_applications_access_stage_attempt_builder, dataproc_projects_locations_batches_spark_applications_access_stage_attempt_task,
    dataproc_projects_locations_batches_spark_applications_access_stage_rdd_graph_builder, dataproc_projects_locations_batches_spark_applications_access_stage_rdd_graph_task,
    dataproc_projects_locations_batches_spark_applications_search_builder, dataproc_projects_locations_batches_spark_applications_search_task,
    dataproc_projects_locations_batches_spark_applications_search_executor_stage_summary_builder, dataproc_projects_locations_batches_spark_applications_search_executor_stage_summary_task,
    dataproc_projects_locations_batches_spark_applications_search_executors_builder, dataproc_projects_locations_batches_spark_applications_search_executors_task,
    dataproc_projects_locations_batches_spark_applications_search_jobs_builder, dataproc_projects_locations_batches_spark_applications_search_jobs_task,
    dataproc_projects_locations_batches_spark_applications_search_sql_queries_builder, dataproc_projects_locations_batches_spark_applications_search_sql_queries_task,
    dataproc_projects_locations_batches_spark_applications_search_stage_attempt_tasks_builder, dataproc_projects_locations_batches_spark_applications_search_stage_attempt_tasks_task,
    dataproc_projects_locations_batches_spark_applications_search_stage_attempts_builder, dataproc_projects_locations_batches_spark_applications_search_stage_attempts_task,
    dataproc_projects_locations_batches_spark_applications_search_stages_builder, dataproc_projects_locations_batches_spark_applications_search_stages_task,
    dataproc_projects_locations_batches_spark_applications_summarize_executors_builder, dataproc_projects_locations_batches_spark_applications_summarize_executors_task,
    dataproc_projects_locations_batches_spark_applications_summarize_jobs_builder, dataproc_projects_locations_batches_spark_applications_summarize_jobs_task,
    dataproc_projects_locations_batches_spark_applications_summarize_stage_attempt_tasks_builder, dataproc_projects_locations_batches_spark_applications_summarize_stage_attempt_tasks_task,
    dataproc_projects_locations_batches_spark_applications_summarize_stages_builder, dataproc_projects_locations_batches_spark_applications_summarize_stages_task,
    dataproc_projects_locations_batches_spark_applications_write_builder, dataproc_projects_locations_batches_spark_applications_write_task,
    dataproc_projects_locations_operations_cancel_builder, dataproc_projects_locations_operations_cancel_task,
    dataproc_projects_locations_operations_delete_builder, dataproc_projects_locations_operations_delete_task,
    dataproc_projects_locations_operations_get_builder, dataproc_projects_locations_operations_get_task,
    dataproc_projects_locations_operations_list_builder, dataproc_projects_locations_operations_list_task,
    dataproc_projects_locations_session_templates_create_builder, dataproc_projects_locations_session_templates_create_task,
    dataproc_projects_locations_session_templates_delete_builder, dataproc_projects_locations_session_templates_delete_task,
    dataproc_projects_locations_session_templates_get_builder, dataproc_projects_locations_session_templates_get_task,
    dataproc_projects_locations_session_templates_list_builder, dataproc_projects_locations_session_templates_list_task,
    dataproc_projects_locations_session_templates_patch_builder, dataproc_projects_locations_session_templates_patch_task,
    dataproc_projects_locations_sessions_create_builder, dataproc_projects_locations_sessions_create_task,
    dataproc_projects_locations_sessions_delete_builder, dataproc_projects_locations_sessions_delete_task,
    dataproc_projects_locations_sessions_get_builder, dataproc_projects_locations_sessions_get_task,
    dataproc_projects_locations_sessions_list_builder, dataproc_projects_locations_sessions_list_task,
    dataproc_projects_locations_sessions_terminate_builder, dataproc_projects_locations_sessions_terminate_task,
    dataproc_projects_locations_sessions_spark_applications_access_builder, dataproc_projects_locations_sessions_spark_applications_access_task,
    dataproc_projects_locations_sessions_spark_applications_access_environment_info_builder, dataproc_projects_locations_sessions_spark_applications_access_environment_info_task,
    dataproc_projects_locations_sessions_spark_applications_access_job_builder, dataproc_projects_locations_sessions_spark_applications_access_job_task,
    dataproc_projects_locations_sessions_spark_applications_access_sql_plan_builder, dataproc_projects_locations_sessions_spark_applications_access_sql_plan_task,
    dataproc_projects_locations_sessions_spark_applications_access_sql_query_builder, dataproc_projects_locations_sessions_spark_applications_access_sql_query_task,
    dataproc_projects_locations_sessions_spark_applications_access_stage_attempt_builder, dataproc_projects_locations_sessions_spark_applications_access_stage_attempt_task,
    dataproc_projects_locations_sessions_spark_applications_access_stage_rdd_graph_builder, dataproc_projects_locations_sessions_spark_applications_access_stage_rdd_graph_task,
    dataproc_projects_locations_sessions_spark_applications_search_builder, dataproc_projects_locations_sessions_spark_applications_search_task,
    dataproc_projects_locations_sessions_spark_applications_search_executor_stage_summary_builder, dataproc_projects_locations_sessions_spark_applications_search_executor_stage_summary_task,
    dataproc_projects_locations_sessions_spark_applications_search_executors_builder, dataproc_projects_locations_sessions_spark_applications_search_executors_task,
    dataproc_projects_locations_sessions_spark_applications_search_jobs_builder, dataproc_projects_locations_sessions_spark_applications_search_jobs_task,
    dataproc_projects_locations_sessions_spark_applications_search_sql_queries_builder, dataproc_projects_locations_sessions_spark_applications_search_sql_queries_task,
    dataproc_projects_locations_sessions_spark_applications_search_stage_attempt_tasks_builder, dataproc_projects_locations_sessions_spark_applications_search_stage_attempt_tasks_task,
    dataproc_projects_locations_sessions_spark_applications_search_stage_attempts_builder, dataproc_projects_locations_sessions_spark_applications_search_stage_attempts_task,
    dataproc_projects_locations_sessions_spark_applications_search_stages_builder, dataproc_projects_locations_sessions_spark_applications_search_stages_task,
    dataproc_projects_locations_sessions_spark_applications_summarize_executors_builder, dataproc_projects_locations_sessions_spark_applications_summarize_executors_task,
    dataproc_projects_locations_sessions_spark_applications_summarize_jobs_builder, dataproc_projects_locations_sessions_spark_applications_summarize_jobs_task,
    dataproc_projects_locations_sessions_spark_applications_summarize_stage_attempt_tasks_builder, dataproc_projects_locations_sessions_spark_applications_summarize_stage_attempt_tasks_task,
    dataproc_projects_locations_sessions_spark_applications_summarize_stages_builder, dataproc_projects_locations_sessions_spark_applications_summarize_stages_task,
    dataproc_projects_locations_sessions_spark_applications_write_builder, dataproc_projects_locations_sessions_spark_applications_write_task,
    dataproc_projects_locations_workflow_templates_create_builder, dataproc_projects_locations_workflow_templates_create_task,
    dataproc_projects_locations_workflow_templates_delete_builder, dataproc_projects_locations_workflow_templates_delete_task,
    dataproc_projects_locations_workflow_templates_get_builder, dataproc_projects_locations_workflow_templates_get_task,
    dataproc_projects_locations_workflow_templates_get_iam_policy_builder, dataproc_projects_locations_workflow_templates_get_iam_policy_task,
    dataproc_projects_locations_workflow_templates_instantiate_builder, dataproc_projects_locations_workflow_templates_instantiate_task,
    dataproc_projects_locations_workflow_templates_instantiate_inline_builder, dataproc_projects_locations_workflow_templates_instantiate_inline_task,
    dataproc_projects_locations_workflow_templates_list_builder, dataproc_projects_locations_workflow_templates_list_task,
    dataproc_projects_locations_workflow_templates_set_iam_policy_builder, dataproc_projects_locations_workflow_templates_set_iam_policy_task,
    dataproc_projects_locations_workflow_templates_test_iam_permissions_builder, dataproc_projects_locations_workflow_templates_test_iam_permissions_task,
    dataproc_projects_locations_workflow_templates_update_builder, dataproc_projects_locations_workflow_templates_update_task,
    dataproc_projects_regions_autoscaling_policies_create_builder, dataproc_projects_regions_autoscaling_policies_create_task,
    dataproc_projects_regions_autoscaling_policies_delete_builder, dataproc_projects_regions_autoscaling_policies_delete_task,
    dataproc_projects_regions_autoscaling_policies_get_builder, dataproc_projects_regions_autoscaling_policies_get_task,
    dataproc_projects_regions_autoscaling_policies_get_iam_policy_builder, dataproc_projects_regions_autoscaling_policies_get_iam_policy_task,
    dataproc_projects_regions_autoscaling_policies_list_builder, dataproc_projects_regions_autoscaling_policies_list_task,
    dataproc_projects_regions_autoscaling_policies_set_iam_policy_builder, dataproc_projects_regions_autoscaling_policies_set_iam_policy_task,
    dataproc_projects_regions_autoscaling_policies_test_iam_permissions_builder, dataproc_projects_regions_autoscaling_policies_test_iam_permissions_task,
    dataproc_projects_regions_autoscaling_policies_update_builder, dataproc_projects_regions_autoscaling_policies_update_task,
    dataproc_projects_regions_clusters_create_builder, dataproc_projects_regions_clusters_create_task,
    dataproc_projects_regions_clusters_delete_builder, dataproc_projects_regions_clusters_delete_task,
    dataproc_projects_regions_clusters_diagnose_builder, dataproc_projects_regions_clusters_diagnose_task,
    dataproc_projects_regions_clusters_get_builder, dataproc_projects_regions_clusters_get_task,
    dataproc_projects_regions_clusters_get_iam_policy_builder, dataproc_projects_regions_clusters_get_iam_policy_task,
    dataproc_projects_regions_clusters_inject_credentials_builder, dataproc_projects_regions_clusters_inject_credentials_task,
    dataproc_projects_regions_clusters_list_builder, dataproc_projects_regions_clusters_list_task,
    dataproc_projects_regions_clusters_patch_builder, dataproc_projects_regions_clusters_patch_task,
    dataproc_projects_regions_clusters_repair_builder, dataproc_projects_regions_clusters_repair_task,
    dataproc_projects_regions_clusters_set_iam_policy_builder, dataproc_projects_regions_clusters_set_iam_policy_task,
    dataproc_projects_regions_clusters_start_builder, dataproc_projects_regions_clusters_start_task,
    dataproc_projects_regions_clusters_stop_builder, dataproc_projects_regions_clusters_stop_task,
    dataproc_projects_regions_clusters_test_iam_permissions_builder, dataproc_projects_regions_clusters_test_iam_permissions_task,
    dataproc_projects_regions_clusters_node_groups_create_builder, dataproc_projects_regions_clusters_node_groups_create_task,
    dataproc_projects_regions_clusters_node_groups_get_builder, dataproc_projects_regions_clusters_node_groups_get_task,
    dataproc_projects_regions_clusters_node_groups_repair_builder, dataproc_projects_regions_clusters_node_groups_repair_task,
    dataproc_projects_regions_clusters_node_groups_resize_builder, dataproc_projects_regions_clusters_node_groups_resize_task,
    dataproc_projects_regions_jobs_cancel_builder, dataproc_projects_regions_jobs_cancel_task,
    dataproc_projects_regions_jobs_delete_builder, dataproc_projects_regions_jobs_delete_task,
    dataproc_projects_regions_jobs_get_builder, dataproc_projects_regions_jobs_get_task,
    dataproc_projects_regions_jobs_get_iam_policy_builder, dataproc_projects_regions_jobs_get_iam_policy_task,
    dataproc_projects_regions_jobs_list_builder, dataproc_projects_regions_jobs_list_task,
    dataproc_projects_regions_jobs_patch_builder, dataproc_projects_regions_jobs_patch_task,
    dataproc_projects_regions_jobs_set_iam_policy_builder, dataproc_projects_regions_jobs_set_iam_policy_task,
    dataproc_projects_regions_jobs_submit_builder, dataproc_projects_regions_jobs_submit_task,
    dataproc_projects_regions_jobs_submit_as_operation_builder, dataproc_projects_regions_jobs_submit_as_operation_task,
    dataproc_projects_regions_jobs_test_iam_permissions_builder, dataproc_projects_regions_jobs_test_iam_permissions_task,
    dataproc_projects_regions_operations_cancel_builder, dataproc_projects_regions_operations_cancel_task,
    dataproc_projects_regions_operations_delete_builder, dataproc_projects_regions_operations_delete_task,
    dataproc_projects_regions_operations_get_builder, dataproc_projects_regions_operations_get_task,
    dataproc_projects_regions_operations_get_iam_policy_builder, dataproc_projects_regions_operations_get_iam_policy_task,
    dataproc_projects_regions_operations_list_builder, dataproc_projects_regions_operations_list_task,
    dataproc_projects_regions_operations_set_iam_policy_builder, dataproc_projects_regions_operations_set_iam_policy_task,
    dataproc_projects_regions_operations_test_iam_permissions_builder, dataproc_projects_regions_operations_test_iam_permissions_task,
    dataproc_projects_regions_workflow_templates_create_builder, dataproc_projects_regions_workflow_templates_create_task,
    dataproc_projects_regions_workflow_templates_delete_builder, dataproc_projects_regions_workflow_templates_delete_task,
    dataproc_projects_regions_workflow_templates_get_builder, dataproc_projects_regions_workflow_templates_get_task,
    dataproc_projects_regions_workflow_templates_get_iam_policy_builder, dataproc_projects_regions_workflow_templates_get_iam_policy_task,
    dataproc_projects_regions_workflow_templates_instantiate_builder, dataproc_projects_regions_workflow_templates_instantiate_task,
    dataproc_projects_regions_workflow_templates_instantiate_inline_builder, dataproc_projects_regions_workflow_templates_instantiate_inline_task,
    dataproc_projects_regions_workflow_templates_list_builder, dataproc_projects_regions_workflow_templates_list_task,
    dataproc_projects_regions_workflow_templates_set_iam_policy_builder, dataproc_projects_regions_workflow_templates_set_iam_policy_task,
    dataproc_projects_regions_workflow_templates_test_iam_permissions_builder, dataproc_projects_regions_workflow_templates_test_iam_permissions_task,
    dataproc_projects_regions_workflow_templates_update_builder, dataproc_projects_regions_workflow_templates_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dataproc::AccessSessionSparkApplicationEnvironmentInfoResponse;
use crate::providers::gcp::clients::dataproc::AccessSessionSparkApplicationJobResponse;
use crate::providers::gcp::clients::dataproc::AccessSessionSparkApplicationResponse;
use crate::providers::gcp::clients::dataproc::AccessSessionSparkApplicationSqlQueryResponse;
use crate::providers::gcp::clients::dataproc::AccessSessionSparkApplicationSqlSparkPlanGraphResponse;
use crate::providers::gcp::clients::dataproc::AccessSessionSparkApplicationStageAttemptResponse;
use crate::providers::gcp::clients::dataproc::AccessSessionSparkApplicationStageRddOperationGraphResponse;
use crate::providers::gcp::clients::dataproc::AccessSparkApplicationEnvironmentInfoResponse;
use crate::providers::gcp::clients::dataproc::AccessSparkApplicationJobResponse;
use crate::providers::gcp::clients::dataproc::AccessSparkApplicationResponse;
use crate::providers::gcp::clients::dataproc::AccessSparkApplicationSqlQueryResponse;
use crate::providers::gcp::clients::dataproc::AccessSparkApplicationSqlSparkPlanGraphResponse;
use crate::providers::gcp::clients::dataproc::AccessSparkApplicationStageAttemptResponse;
use crate::providers::gcp::clients::dataproc::AccessSparkApplicationStageRddOperationGraphResponse;
use crate::providers::gcp::clients::dataproc::AutoscalingPolicy;
use crate::providers::gcp::clients::dataproc::Batch;
use crate::providers::gcp::clients::dataproc::Cluster;
use crate::providers::gcp::clients::dataproc::Empty;
use crate::providers::gcp::clients::dataproc::Job;
use crate::providers::gcp::clients::dataproc::ListAutoscalingPoliciesResponse;
use crate::providers::gcp::clients::dataproc::ListBatchesResponse;
use crate::providers::gcp::clients::dataproc::ListClustersResponse;
use crate::providers::gcp::clients::dataproc::ListJobsResponse;
use crate::providers::gcp::clients::dataproc::ListOperationsResponse;
use crate::providers::gcp::clients::dataproc::ListSessionTemplatesResponse;
use crate::providers::gcp::clients::dataproc::ListSessionsResponse;
use crate::providers::gcp::clients::dataproc::ListWorkflowTemplatesResponse;
use crate::providers::gcp::clients::dataproc::NodeGroup;
use crate::providers::gcp::clients::dataproc::Operation;
use crate::providers::gcp::clients::dataproc::Policy;
use crate::providers::gcp::clients::dataproc::SearchSessionSparkApplicationExecutorStageSummaryResponse;
use crate::providers::gcp::clients::dataproc::SearchSessionSparkApplicationExecutorsResponse;
use crate::providers::gcp::clients::dataproc::SearchSessionSparkApplicationJobsResponse;
use crate::providers::gcp::clients::dataproc::SearchSessionSparkApplicationSqlQueriesResponse;
use crate::providers::gcp::clients::dataproc::SearchSessionSparkApplicationStageAttemptTasksResponse;
use crate::providers::gcp::clients::dataproc::SearchSessionSparkApplicationStageAttemptsResponse;
use crate::providers::gcp::clients::dataproc::SearchSessionSparkApplicationStagesResponse;
use crate::providers::gcp::clients::dataproc::SearchSessionSparkApplicationsResponse;
use crate::providers::gcp::clients::dataproc::SearchSparkApplicationExecutorStageSummaryResponse;
use crate::providers::gcp::clients::dataproc::SearchSparkApplicationExecutorsResponse;
use crate::providers::gcp::clients::dataproc::SearchSparkApplicationJobsResponse;
use crate::providers::gcp::clients::dataproc::SearchSparkApplicationSqlQueriesResponse;
use crate::providers::gcp::clients::dataproc::SearchSparkApplicationStageAttemptTasksResponse;
use crate::providers::gcp::clients::dataproc::SearchSparkApplicationStageAttemptsResponse;
use crate::providers::gcp::clients::dataproc::SearchSparkApplicationStagesResponse;
use crate::providers::gcp::clients::dataproc::SearchSparkApplicationsResponse;
use crate::providers::gcp::clients::dataproc::Session;
use crate::providers::gcp::clients::dataproc::SessionTemplate;
use crate::providers::gcp::clients::dataproc::SummarizeSessionSparkApplicationExecutorsResponse;
use crate::providers::gcp::clients::dataproc::SummarizeSessionSparkApplicationJobsResponse;
use crate::providers::gcp::clients::dataproc::SummarizeSessionSparkApplicationStageAttemptTasksResponse;
use crate::providers::gcp::clients::dataproc::SummarizeSessionSparkApplicationStagesResponse;
use crate::providers::gcp::clients::dataproc::SummarizeSparkApplicationExecutorsResponse;
use crate::providers::gcp::clients::dataproc::SummarizeSparkApplicationJobsResponse;
use crate::providers::gcp::clients::dataproc::SummarizeSparkApplicationStageAttemptTasksResponse;
use crate::providers::gcp::clients::dataproc::SummarizeSparkApplicationStagesResponse;
use crate::providers::gcp::clients::dataproc::TestIamPermissionsResponse;
use crate::providers::gcp::clients::dataproc::WorkflowTemplate;
use crate::providers::gcp::clients::dataproc::WriteSessionSparkApplicationContextResponse;
use crate::providers::gcp::clients::dataproc::WriteSparkApplicationContextResponse;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesUpdateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesAnalyzeArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsAccessArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsAccessEnvironmentInfoArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsAccessJobArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsAccessSqlPlanArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsAccessSqlQueryArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsAccessStageAttemptArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsAccessStageRddGraphArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSearchArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSearchExecutorStageSummaryArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSearchExecutorsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSearchJobsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSearchSqlQueriesArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSearchStageAttemptTasksArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSearchStageAttemptsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSearchStagesArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSummarizeExecutorsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSummarizeJobsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSummarizeStageAttemptTasksArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsSummarizeStagesArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsWriteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionTemplatesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionTemplatesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionTemplatesGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionTemplatesListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionTemplatesPatchArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsAccessArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsAccessEnvironmentInfoArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsAccessJobArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsAccessSqlPlanArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsAccessSqlQueryArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsAccessStageAttemptArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsAccessStageRddGraphArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSearchArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSearchExecutorStageSummaryArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSearchExecutorsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSearchJobsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSearchSqlQueriesArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSearchStageAttemptTasksArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSearchStageAttemptsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSearchStagesArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSummarizeExecutorsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSummarizeJobsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSummarizeStageAttemptTasksArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsSummarizeStagesArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsWriteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsTerminateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesInstantiateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesInstantiateInlineArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesUpdateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesUpdateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersDiagnoseArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersInjectCredentialsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersNodeGroupsCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersNodeGroupsGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersNodeGroupsRepairArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersNodeGroupsResizeArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersPatchArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersRepairArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersStartArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersStopArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsCancelArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsPatchArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsSubmitArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsSubmitAsOperationArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsCancelArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesGetArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesInstantiateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesInstantiateInlineArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesListArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DataprocProvider with automatic state tracking.
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
/// let provider = DataprocProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct DataprocProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> DataprocProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new DataprocProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new DataprocProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Dataproc projects locations autoscaling policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_autoscaling_policies_create(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies delete.
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
    pub fn dataproc_projects_locations_autoscaling_policies_delete(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_autoscaling_policies_get(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_autoscaling_policies_get_iam_policy(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAutoscalingPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_autoscaling_policies_list(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAutoscalingPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_autoscaling_policies_set_iam_policy(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_autoscaling_policies_test_iam_permissions(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_autoscaling_policies_update(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches analyze.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_analyze(
        &self,
        args: &DataprocProjectsLocationsBatchesAnalyzeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_analyze_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_analyze_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_batches_create(
        &self,
        args: &DataprocProjectsLocationsBatchesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_create_builder(
            &self.http_client,
            &args.parent,
            &args.batchId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches delete.
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
    pub fn dataproc_projects_locations_batches_delete(
        &self,
        args: &DataprocProjectsLocationsBatchesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Batch result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_get(
        &self,
        args: &DataprocProjectsLocationsBatchesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Batch, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBatchesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_list(
        &self,
        args: &DataprocProjectsLocationsBatchesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBatchesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications access.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSparkApplicationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_access(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSparkApplicationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_access_builder(
            &self.http_client,
            &args.name,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_access_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications access environment info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSparkApplicationEnvironmentInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_access_environment_info(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsAccessEnvironmentInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSparkApplicationEnvironmentInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_access_environment_info_builder(
            &self.http_client,
            &args.name,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_access_environment_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications access job.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSparkApplicationJobResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_access_job(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsAccessJobArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSparkApplicationJobResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_access_job_builder(
            &self.http_client,
            &args.name,
            &args.jobId,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_access_job_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications access sql plan.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSparkApplicationSqlSparkPlanGraphResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_access_sql_plan(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsAccessSqlPlanArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSparkApplicationSqlSparkPlanGraphResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_access_sql_plan_builder(
            &self.http_client,
            &args.name,
            &args.executionId,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_access_sql_plan_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications access sql query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSparkApplicationSqlQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_access_sql_query(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsAccessSqlQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSparkApplicationSqlQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_access_sql_query_builder(
            &self.http_client,
            &args.name,
            &args.details,
            &args.executionId,
            &args.parent,
            &args.planDescription,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_access_sql_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications access stage attempt.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSparkApplicationStageAttemptResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_access_stage_attempt(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsAccessStageAttemptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSparkApplicationStageAttemptResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_access_stage_attempt_builder(
            &self.http_client,
            &args.name,
            &args.parent,
            &args.stageAttemptId,
            &args.stageId,
            &args.summaryMetricsMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_access_stage_attempt_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications access stage rdd graph.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSparkApplicationStageRddOperationGraphResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_access_stage_rdd_graph(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsAccessStageRddGraphArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSparkApplicationStageRddOperationGraphResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_access_stage_rdd_graph_builder(
            &self.http_client,
            &args.name,
            &args.parent,
            &args.stageId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_access_stage_rdd_graph_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSparkApplicationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_search(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSparkApplicationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_search_builder(
            &self.http_client,
            &args.parent,
            &args.applicationStatus,
            &args.maxEndTime,
            &args.maxTime,
            &args.minEndTime,
            &args.minTime,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications search executor stage summary.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSparkApplicationExecutorStageSummaryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_search_executor_stage_summary(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSearchExecutorStageSummaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSparkApplicationExecutorStageSummaryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_search_executor_stage_summary_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.stageAttemptId,
            &args.stageId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_search_executor_stage_summary_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications search executors.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSparkApplicationExecutorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_search_executors(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSearchExecutorsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSparkApplicationExecutorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_search_executors_builder(
            &self.http_client,
            &args.name,
            &args.executorStatus,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_search_executors_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications search jobs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSparkApplicationJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_search_jobs(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSearchJobsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSparkApplicationJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_search_jobs_builder(
            &self.http_client,
            &args.name,
            &args.jobStatus,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_search_jobs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications search sql queries.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSparkApplicationSqlQueriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_search_sql_queries(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSearchSqlQueriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSparkApplicationSqlQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_search_sql_queries_builder(
            &self.http_client,
            &args.name,
            &args.details,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.planDescription,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_search_sql_queries_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications search stage attempt tasks.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSparkApplicationStageAttemptTasksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_search_stage_attempt_tasks(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSearchStageAttemptTasksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSparkApplicationStageAttemptTasksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_search_stage_attempt_tasks_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.sortRuntime,
            &args.stageAttemptId,
            &args.stageId,
            &args.taskStatus,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_search_stage_attempt_tasks_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications search stage attempts.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSparkApplicationStageAttemptsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_search_stage_attempts(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSearchStageAttemptsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSparkApplicationStageAttemptsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_search_stage_attempts_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.stageId,
            &args.summaryMetricsMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_search_stage_attempts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications search stages.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSparkApplicationStagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_search_stages(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSearchStagesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSparkApplicationStagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_search_stages_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.stageStatus,
            &args.summaryMetricsMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_search_stages_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications summarize executors.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SummarizeSparkApplicationExecutorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_summarize_executors(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSummarizeExecutorsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SummarizeSparkApplicationExecutorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_summarize_executors_builder(
            &self.http_client,
            &args.name,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_summarize_executors_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications summarize jobs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SummarizeSparkApplicationJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_summarize_jobs(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSummarizeJobsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SummarizeSparkApplicationJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_summarize_jobs_builder(
            &self.http_client,
            &args.name,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_summarize_jobs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications summarize stage attempt tasks.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SummarizeSparkApplicationStageAttemptTasksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_summarize_stage_attempt_tasks(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSummarizeStageAttemptTasksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SummarizeSparkApplicationStageAttemptTasksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_summarize_stage_attempt_tasks_builder(
            &self.http_client,
            &args.name,
            &args.parent,
            &args.stageAttemptId,
            &args.stageId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_summarize_stage_attempt_tasks_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications summarize stages.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SummarizeSparkApplicationStagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_batches_spark_applications_summarize_stages(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsSummarizeStagesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SummarizeSparkApplicationStagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_summarize_stages_builder(
            &self.http_client,
            &args.name,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_summarize_stages_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WriteSparkApplicationContextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_batches_spark_applications_write(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WriteSparkApplicationContextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_write_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations operations cancel.
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
    pub fn dataproc_projects_locations_operations_cancel(
        &self,
        args: &DataprocProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations operations delete.
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
    pub fn dataproc_projects_locations_operations_delete(
        &self,
        args: &DataprocProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_operations_get(
        &self,
        args: &DataprocProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_operations_list(
        &self,
        args: &DataprocProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations session templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SessionTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_session_templates_create(
        &self,
        args: &DataprocProjectsLocationsSessionTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SessionTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_session_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_session_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations session templates delete.
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
    pub fn dataproc_projects_locations_session_templates_delete(
        &self,
        args: &DataprocProjectsLocationsSessionTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_session_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_session_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations session templates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SessionTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_session_templates_get(
        &self,
        args: &DataprocProjectsLocationsSessionTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SessionTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_session_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_session_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations session templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSessionTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_session_templates_list(
        &self,
        args: &DataprocProjectsLocationsSessionTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSessionTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_session_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_session_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations session templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SessionTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_session_templates_patch(
        &self,
        args: &DataprocProjectsLocationsSessionTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SessionTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_session_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_session_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_sessions_create(
        &self,
        args: &DataprocProjectsLocationsSessionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.sessionId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_sessions_delete(
        &self,
        args: &DataprocProjectsLocationsSessionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Session result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_get(
        &self,
        args: &DataprocProjectsLocationsSessionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Session, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSessionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_list(
        &self,
        args: &DataprocProjectsLocationsSessionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSessionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions terminate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_sessions_terminate(
        &self,
        args: &DataprocProjectsLocationsSessionsTerminateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_terminate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_terminate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications access.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSessionSparkApplicationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_access(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSessionSparkApplicationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_access_builder(
            &self.http_client,
            &args.name,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_access_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications access environment info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSessionSparkApplicationEnvironmentInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_access_environment_info(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsAccessEnvironmentInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSessionSparkApplicationEnvironmentInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_access_environment_info_builder(
            &self.http_client,
            &args.name,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_access_environment_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications access job.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSessionSparkApplicationJobResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_access_job(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsAccessJobArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSessionSparkApplicationJobResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_access_job_builder(
            &self.http_client,
            &args.name,
            &args.jobId,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_access_job_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications access sql plan.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSessionSparkApplicationSqlSparkPlanGraphResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_access_sql_plan(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsAccessSqlPlanArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSessionSparkApplicationSqlSparkPlanGraphResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_access_sql_plan_builder(
            &self.http_client,
            &args.name,
            &args.executionId,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_access_sql_plan_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications access sql query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSessionSparkApplicationSqlQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_access_sql_query(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsAccessSqlQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSessionSparkApplicationSqlQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_access_sql_query_builder(
            &self.http_client,
            &args.name,
            &args.details,
            &args.executionId,
            &args.parent,
            &args.planDescription,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_access_sql_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications access stage attempt.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSessionSparkApplicationStageAttemptResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_access_stage_attempt(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsAccessStageAttemptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSessionSparkApplicationStageAttemptResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_access_stage_attempt_builder(
            &self.http_client,
            &args.name,
            &args.parent,
            &args.stageAttemptId,
            &args.stageId,
            &args.summaryMetricsMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_access_stage_attempt_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications access stage rdd graph.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSessionSparkApplicationStageRddOperationGraphResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_access_stage_rdd_graph(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsAccessStageRddGraphArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSessionSparkApplicationStageRddOperationGraphResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_access_stage_rdd_graph_builder(
            &self.http_client,
            &args.name,
            &args.parent,
            &args.stageId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_access_stage_rdd_graph_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSessionSparkApplicationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_search(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSessionSparkApplicationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_search_builder(
            &self.http_client,
            &args.parent,
            &args.applicationStatus,
            &args.maxEndTime,
            &args.maxTime,
            &args.minEndTime,
            &args.minTime,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications search executor stage summary.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSessionSparkApplicationExecutorStageSummaryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_search_executor_stage_summary(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSearchExecutorStageSummaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSessionSparkApplicationExecutorStageSummaryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_search_executor_stage_summary_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.stageAttemptId,
            &args.stageId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_search_executor_stage_summary_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications search executors.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSessionSparkApplicationExecutorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_search_executors(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSearchExecutorsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSessionSparkApplicationExecutorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_search_executors_builder(
            &self.http_client,
            &args.name,
            &args.executorStatus,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_search_executors_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications search jobs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSessionSparkApplicationJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_search_jobs(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSearchJobsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSessionSparkApplicationJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_search_jobs_builder(
            &self.http_client,
            &args.name,
            &args.jobIds,
            &args.jobStatus,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_search_jobs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications search sql queries.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSessionSparkApplicationSqlQueriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_search_sql_queries(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSearchSqlQueriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSessionSparkApplicationSqlQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_search_sql_queries_builder(
            &self.http_client,
            &args.name,
            &args.details,
            &args.operationIds,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.planDescription,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_search_sql_queries_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications search stage attempt tasks.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSessionSparkApplicationStageAttemptTasksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_search_stage_attempt_tasks(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSearchStageAttemptTasksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSessionSparkApplicationStageAttemptTasksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_search_stage_attempt_tasks_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.sortRuntime,
            &args.stageAttemptId,
            &args.stageId,
            &args.taskStatus,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_search_stage_attempt_tasks_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications search stage attempts.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSessionSparkApplicationStageAttemptsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_search_stage_attempts(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSearchStageAttemptsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSessionSparkApplicationStageAttemptsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_search_stage_attempts_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.stageId,
            &args.summaryMetricsMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_search_stage_attempts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications search stages.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSessionSparkApplicationStagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_search_stages(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSearchStagesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSessionSparkApplicationStagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_search_stages_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.stageIds,
            &args.stageStatus,
            &args.summaryMetricsMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_search_stages_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications summarize executors.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SummarizeSessionSparkApplicationExecutorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_summarize_executors(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSummarizeExecutorsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SummarizeSessionSparkApplicationExecutorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_summarize_executors_builder(
            &self.http_client,
            &args.name,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_summarize_executors_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications summarize jobs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SummarizeSessionSparkApplicationJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_summarize_jobs(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSummarizeJobsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SummarizeSessionSparkApplicationJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_summarize_jobs_builder(
            &self.http_client,
            &args.name,
            &args.jobIds,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_summarize_jobs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications summarize stage attempt tasks.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SummarizeSessionSparkApplicationStageAttemptTasksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_summarize_stage_attempt_tasks(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSummarizeStageAttemptTasksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SummarizeSessionSparkApplicationStageAttemptTasksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_summarize_stage_attempt_tasks_builder(
            &self.http_client,
            &args.name,
            &args.parent,
            &args.stageAttemptId,
            &args.stageId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_summarize_stage_attempt_tasks_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications summarize stages.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SummarizeSessionSparkApplicationStagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_summarize_stages(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsSummarizeStagesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SummarizeSessionSparkApplicationStagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_summarize_stages_builder(
            &self.http_client,
            &args.name,
            &args.parent,
            &args.stageIds,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_summarize_stages_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WriteSessionSparkApplicationContextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_write(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WriteSessionSparkApplicationContextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_write_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_workflow_templates_create(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates delete.
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
    pub fn dataproc_projects_locations_workflow_templates_delete(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_delete_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_workflow_templates_get(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_get_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_workflow_templates_get_iam_policy(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates instantiate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_workflow_templates_instantiate(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesInstantiateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_instantiate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_instantiate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates instantiate inline.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_workflow_templates_instantiate_inline(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesInstantiateInlineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_instantiate_inline_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_instantiate_inline_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkflowTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_workflow_templates_list(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkflowTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_workflow_templates_set_iam_policy(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_locations_workflow_templates_test_iam_permissions(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_workflow_templates_update(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_autoscaling_policies_create(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies delete.
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
    pub fn dataproc_projects_regions_autoscaling_policies_delete(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_autoscaling_policies_get(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_autoscaling_policies_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAutoscalingPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_autoscaling_policies_list(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAutoscalingPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_autoscaling_policies_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_autoscaling_policies_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_autoscaling_policies_update(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_create(
        &self,
        args: &DataprocProjectsRegionsClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_create_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.actionOnFailedPrimaryWorkers,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_delete(
        &self,
        args: &DataprocProjectsRegionsClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
            &args.clusterUuid,
            &args.gracefulTerminationTimeout,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters diagnose.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_diagnose(
        &self,
        args: &DataprocProjectsRegionsClustersDiagnoseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_diagnose_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_diagnose_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Cluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_clusters_get(
        &self,
        args: &DataprocProjectsRegionsClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Cluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_get_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_clusters_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsClustersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters inject credentials.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_inject_credentials(
        &self,
        args: &DataprocProjectsRegionsClustersInjectCredentialsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_inject_credentials_builder(
            &self.http_client,
            &args.project,
            &args.region,
            &args.cluster,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_inject_credentials_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_clusters_list(
        &self,
        args: &DataprocProjectsRegionsClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_list_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_patch(
        &self,
        args: &DataprocProjectsRegionsClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_patch_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
            &args.gracefulDecommissionTimeout,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters repair.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_repair(
        &self,
        args: &DataprocProjectsRegionsClustersRepairArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_repair_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_repair_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters start.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_start(
        &self,
        args: &DataprocProjectsRegionsClustersStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_start_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_stop(
        &self,
        args: &DataprocProjectsRegionsClustersStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_stop_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_clusters_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters node groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_node_groups_create(
        &self,
        args: &DataprocProjectsRegionsClustersNodeGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_node_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.nodeGroupId,
            &args.parentOperationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_node_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters node groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NodeGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_clusters_node_groups_get(
        &self,
        args: &DataprocProjectsRegionsClustersNodeGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NodeGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_node_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_node_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters node groups repair.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_node_groups_repair(
        &self,
        args: &DataprocProjectsRegionsClustersNodeGroupsRepairArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_node_groups_repair_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_node_groups_repair_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters node groups resize.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_clusters_node_groups_resize(
        &self,
        args: &DataprocProjectsRegionsClustersNodeGroupsResizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_node_groups_resize_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_node_groups_resize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_jobs_cancel(
        &self,
        args: &DataprocProjectsRegionsJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_cancel_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs delete.
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
    pub fn dataproc_projects_regions_jobs_delete(
        &self,
        args: &DataprocProjectsRegionsJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_jobs_get(
        &self,
        args: &DataprocProjectsRegionsJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_get_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_jobs_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsJobsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_jobs_list(
        &self,
        args: &DataprocProjectsRegionsJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_list_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
            &args.filter,
            &args.jobStateMatcher,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_jobs_patch(
        &self,
        args: &DataprocProjectsRegionsJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_patch_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.jobId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_jobs_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsJobsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs submit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_jobs_submit(
        &self,
        args: &DataprocProjectsRegionsJobsSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_submit_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_submit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs submit as operation.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_jobs_submit_as_operation(
        &self,
        args: &DataprocProjectsRegionsJobsSubmitAsOperationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_submit_as_operation_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_submit_as_operation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_jobs_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsJobsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations cancel.
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
    pub fn dataproc_projects_regions_operations_cancel(
        &self,
        args: &DataprocProjectsRegionsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations delete.
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
    pub fn dataproc_projects_regions_operations_delete(
        &self,
        args: &DataprocProjectsRegionsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_operations_get(
        &self,
        args: &DataprocProjectsRegionsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_operations_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsOperationsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_operations_list(
        &self,
        args: &DataprocProjectsRegionsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_operations_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsOperationsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_operations_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsOperationsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_workflow_templates_create(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates delete.
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
    pub fn dataproc_projects_regions_workflow_templates_delete(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_delete_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_workflow_templates_get(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_get_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_workflow_templates_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates instantiate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_workflow_templates_instantiate(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesInstantiateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_instantiate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_instantiate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates instantiate inline.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_workflow_templates_instantiate_inline(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesInstantiateInlineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_instantiate_inline_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_instantiate_inline_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkflowTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_workflow_templates_list(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkflowTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_workflow_templates_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataproc_projects_regions_workflow_templates_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_workflow_templates_update(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
