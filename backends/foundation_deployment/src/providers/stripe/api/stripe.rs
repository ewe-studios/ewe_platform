//! StripeProvider - State-aware stripe API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       stripe API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "stripe")]

use crate::providers::stripe::clients::stripe::{
    post_account_links_builder, post_account_links_task,
    post_account_sessions_builder, post_account_sessions_task,
    post_accounts_builder, post_accounts_task,
    post_accounts_account_builder, post_accounts_account_task,
    delete_accounts_account_builder, delete_accounts_account_task,
    post_accounts_account_bank_accounts_builder, post_accounts_account_bank_accounts_task,
    post_accounts_account_bank_accounts_id_builder, post_accounts_account_bank_accounts_id_task,
    delete_accounts_account_bank_accounts_id_builder, delete_accounts_account_bank_accounts_id_task,
    post_accounts_account_capabilities_capability_builder, post_accounts_account_capabilities_capability_task,
    post_accounts_account_external_accounts_builder, post_accounts_account_external_accounts_task,
    post_accounts_account_external_accounts_id_builder, post_accounts_account_external_accounts_id_task,
    delete_accounts_account_external_accounts_id_builder, delete_accounts_account_external_accounts_id_task,
    post_accounts_account_login_links_builder, post_accounts_account_login_links_task,
    post_accounts_account_people_builder, post_accounts_account_people_task,
    post_accounts_account_people_person_builder, post_accounts_account_people_person_task,
    delete_accounts_account_people_person_builder, delete_accounts_account_people_person_task,
    post_accounts_account_persons_builder, post_accounts_account_persons_task,
    post_accounts_account_persons_person_builder, post_accounts_account_persons_person_task,
    delete_accounts_account_persons_person_builder, delete_accounts_account_persons_person_task,
    post_accounts_account_reject_builder, post_accounts_account_reject_task,
    post_apple_pay_domains_builder, post_apple_pay_domains_task,
    delete_apple_pay_domains_domain_builder, delete_apple_pay_domains_domain_task,
    post_application_fees_fee_refunds_id_builder, post_application_fees_fee_refunds_id_task,
    post_application_fees_id_refund_builder, post_application_fees_id_refund_task,
    post_application_fees_id_refunds_builder, post_application_fees_id_refunds_task,
    post_apps_secrets_builder, post_apps_secrets_task,
    post_apps_secrets_delete_builder, post_apps_secrets_delete_task,
    post_balance_settings_builder, post_balance_settings_task,
    post_billing_alerts_builder, post_billing_alerts_task,
    post_billing_alerts_id_activate_builder, post_billing_alerts_id_activate_task,
    post_billing_alerts_id_archive_builder, post_billing_alerts_id_archive_task,
    post_billing_alerts_id_deactivate_builder, post_billing_alerts_id_deactivate_task,
    post_billing_credit_grants_builder, post_billing_credit_grants_task,
    post_billing_credit_grants_id_builder, post_billing_credit_grants_id_task,
    post_billing_credit_grants_id_expire_builder, post_billing_credit_grants_id_expire_task,
    post_billing_credit_grants_id_void_builder, post_billing_credit_grants_id_void_task,
    post_billing_meter_event_adjustments_builder, post_billing_meter_event_adjustments_task,
    post_billing_meter_events_builder, post_billing_meter_events_task,
    post_billing_meters_builder, post_billing_meters_task,
    post_billing_meters_id_builder, post_billing_meters_id_task,
    post_billing_meters_id_deactivate_builder, post_billing_meters_id_deactivate_task,
    post_billing_meters_id_reactivate_builder, post_billing_meters_id_reactivate_task,
    post_billing_portal_configurations_builder, post_billing_portal_configurations_task,
    post_billing_portal_configurations_configuration_builder, post_billing_portal_configurations_configuration_task,
    post_billing_portal_sessions_builder, post_billing_portal_sessions_task,
    post_charges_builder, post_charges_task,
    post_charges_charge_builder, post_charges_charge_task,
    post_charges_charge_capture_builder, post_charges_charge_capture_task,
    post_charges_charge_dispute_builder, post_charges_charge_dispute_task,
    post_charges_charge_dispute_close_builder, post_charges_charge_dispute_close_task,
    post_charges_charge_refund_builder, post_charges_charge_refund_task,
    post_charges_charge_refunds_builder, post_charges_charge_refunds_task,
    post_charges_charge_refunds_refund_builder, post_charges_charge_refunds_refund_task,
    post_checkout_sessions_builder, post_checkout_sessions_task,
    post_checkout_sessions_session_builder, post_checkout_sessions_session_task,
    post_checkout_sessions_session_expire_builder, post_checkout_sessions_session_expire_task,
    post_climate_orders_builder, post_climate_orders_task,
    post_climate_orders_order_builder, post_climate_orders_order_task,
    post_climate_orders_order_cancel_builder, post_climate_orders_order_cancel_task,
    post_coupons_builder, post_coupons_task,
    post_coupons_coupon_builder, post_coupons_coupon_task,
    delete_coupons_coupon_builder, delete_coupons_coupon_task,
    post_credit_notes_builder, post_credit_notes_task,
    post_credit_notes_id_builder, post_credit_notes_id_task,
    post_credit_notes_id_void_builder, post_credit_notes_id_void_task,
    post_customer_sessions_builder, post_customer_sessions_task,
    post_customers_builder, post_customers_task,
    post_customers_customer_builder, post_customers_customer_task,
    delete_customers_customer_builder, delete_customers_customer_task,
    post_customers_customer_balance_transactions_builder, post_customers_customer_balance_transactions_task,
    post_customers_customer_balance_transactions_transaction_builder, post_customers_customer_balance_transactions_transaction_task,
    post_customers_customer_bank_accounts_builder, post_customers_customer_bank_accounts_task,
    post_customers_customer_bank_accounts_id_builder, post_customers_customer_bank_accounts_id_task,
    delete_customers_customer_bank_accounts_id_builder, delete_customers_customer_bank_accounts_id_task,
    post_customers_customer_bank_accounts_id_verify_builder, post_customers_customer_bank_accounts_id_verify_task,
    post_customers_customer_cards_builder, post_customers_customer_cards_task,
    post_customers_customer_cards_id_builder, post_customers_customer_cards_id_task,
    delete_customers_customer_cards_id_builder, delete_customers_customer_cards_id_task,
    post_customers_customer_cash_balance_builder, post_customers_customer_cash_balance_task,
    delete_customers_customer_discount_builder, delete_customers_customer_discount_task,
    post_customers_customer_funding_instructions_builder, post_customers_customer_funding_instructions_task,
    post_customers_customer_sources_builder, post_customers_customer_sources_task,
    post_customers_customer_sources_id_builder, post_customers_customer_sources_id_task,
    delete_customers_customer_sources_id_builder, delete_customers_customer_sources_id_task,
    post_customers_customer_sources_id_verify_builder, post_customers_customer_sources_id_verify_task,
    post_customers_customer_subscriptions_builder, post_customers_customer_subscriptions_task,
    post_customers_customer_subscriptions_subscription_exposed_id_builder, post_customers_customer_subscriptions_subscription_exposed_id_task,
    delete_customers_customer_subscriptions_subscription_exposed_id_builder, delete_customers_customer_subscriptions_subscription_exposed_id_task,
    delete_customers_customer_subscriptions_subscription_exposed_id_discount_builder, delete_customers_customer_subscriptions_subscription_exposed_id_discount_task,
    post_customers_customer_tax_ids_builder, post_customers_customer_tax_ids_task,
    delete_customers_customer_tax_ids_id_builder, delete_customers_customer_tax_ids_id_task,
    post_disputes_dispute_builder, post_disputes_dispute_task,
    post_disputes_dispute_close_builder, post_disputes_dispute_close_task,
    post_entitlements_features_builder, post_entitlements_features_task,
    post_entitlements_features_id_builder, post_entitlements_features_id_task,
    post_ephemeral_keys_builder, post_ephemeral_keys_task,
    delete_ephemeral_keys_key_builder, delete_ephemeral_keys_key_task,
    post_external_accounts_id_builder, post_external_accounts_id_task,
    post_file_links_builder, post_file_links_task,
    post_file_links_link_builder, post_file_links_link_task,
    post_files_builder, post_files_task,
    post_financial_connections_accounts_account_disconnect_builder, post_financial_connections_accounts_account_disconnect_task,
    post_financial_connections_accounts_account_refresh_builder, post_financial_connections_accounts_account_refresh_task,
    post_financial_connections_accounts_account_subscribe_builder, post_financial_connections_accounts_account_subscribe_task,
    post_financial_connections_accounts_account_unsubscribe_builder, post_financial_connections_accounts_account_unsubscribe_task,
    post_financial_connections_sessions_builder, post_financial_connections_sessions_task,
    post_forwarding_requests_builder, post_forwarding_requests_task,
    post_identity_verification_sessions_builder, post_identity_verification_sessions_task,
    post_identity_verification_sessions_session_builder, post_identity_verification_sessions_session_task,
    post_identity_verification_sessions_session_cancel_builder, post_identity_verification_sessions_session_cancel_task,
    post_identity_verification_sessions_session_redact_builder, post_identity_verification_sessions_session_redact_task,
    post_invoice_rendering_templates_template_archive_builder, post_invoice_rendering_templates_template_archive_task,
    post_invoice_rendering_templates_template_unarchive_builder, post_invoice_rendering_templates_template_unarchive_task,
    post_invoiceitems_builder, post_invoiceitems_task,
    post_invoiceitems_invoiceitem_builder, post_invoiceitems_invoiceitem_task,
    delete_invoiceitems_invoiceitem_builder, delete_invoiceitems_invoiceitem_task,
    post_invoices_builder, post_invoices_task,
    post_invoices_create_preview_builder, post_invoices_create_preview_task,
    post_invoices_invoice_builder, post_invoices_invoice_task,
    delete_invoices_invoice_builder, delete_invoices_invoice_task,
    post_invoices_invoice_add_lines_builder, post_invoices_invoice_add_lines_task,
    post_invoices_invoice_attach_payment_builder, post_invoices_invoice_attach_payment_task,
    post_invoices_invoice_finalize_builder, post_invoices_invoice_finalize_task,
    post_invoices_invoice_lines_line_item_id_builder, post_invoices_invoice_lines_line_item_id_task,
    post_invoices_invoice_mark_uncollectible_builder, post_invoices_invoice_mark_uncollectible_task,
    post_invoices_invoice_pay_builder, post_invoices_invoice_pay_task,
    post_invoices_invoice_remove_lines_builder, post_invoices_invoice_remove_lines_task,
    post_invoices_invoice_send_builder, post_invoices_invoice_send_task,
    post_invoices_invoice_update_lines_builder, post_invoices_invoice_update_lines_task,
    post_invoices_invoice_void_builder, post_invoices_invoice_void_task,
    post_issuing_authorizations_authorization_builder, post_issuing_authorizations_authorization_task,
    post_issuing_authorizations_authorization_approve_builder, post_issuing_authorizations_authorization_approve_task,
    post_issuing_authorizations_authorization_decline_builder, post_issuing_authorizations_authorization_decline_task,
    post_issuing_cardholders_builder, post_issuing_cardholders_task,
    post_issuing_cardholders_cardholder_builder, post_issuing_cardholders_cardholder_task,
    post_issuing_cards_builder, post_issuing_cards_task,
    post_issuing_cards_card_builder, post_issuing_cards_card_task,
    post_issuing_disputes_builder, post_issuing_disputes_task,
    post_issuing_disputes_dispute_builder, post_issuing_disputes_dispute_task,
    post_issuing_disputes_dispute_submit_builder, post_issuing_disputes_dispute_submit_task,
    post_issuing_personalization_designs_builder, post_issuing_personalization_designs_task,
    post_issuing_personalization_designs_personalization_design_builder, post_issuing_personalization_designs_personalization_design_task,
    post_issuing_settlements_settlement_builder, post_issuing_settlements_settlement_task,
    post_issuing_tokens_token_builder, post_issuing_tokens_token_task,
    post_issuing_transactions_transaction_builder, post_issuing_transactions_transaction_task,
    post_link_account_sessions_builder, post_link_account_sessions_task,
    post_linked_accounts_account_disconnect_builder, post_linked_accounts_account_disconnect_task,
    post_linked_accounts_account_refresh_builder, post_linked_accounts_account_refresh_task,
    post_payment_intents_builder, post_payment_intents_task,
    post_payment_intents_intent_builder, post_payment_intents_intent_task,
    post_payment_intents_intent_apply_customer_balance_builder, post_payment_intents_intent_apply_customer_balance_task,
    post_payment_intents_intent_cancel_builder, post_payment_intents_intent_cancel_task,
    post_payment_intents_intent_capture_builder, post_payment_intents_intent_capture_task,
    post_payment_intents_intent_confirm_builder, post_payment_intents_intent_confirm_task,
    post_payment_intents_intent_increment_authorization_builder, post_payment_intents_intent_increment_authorization_task,
    post_payment_intents_intent_verify_microdeposits_builder, post_payment_intents_intent_verify_microdeposits_task,
    post_payment_links_builder, post_payment_links_task,
    post_payment_links_payment_link_builder, post_payment_links_payment_link_task,
    post_payment_method_configurations_builder, post_payment_method_configurations_task,
    post_payment_method_configurations_configuration_builder, post_payment_method_configurations_configuration_task,
    post_payment_method_domains_builder, post_payment_method_domains_task,
    post_payment_method_domains_payment_method_domain_builder, post_payment_method_domains_payment_method_domain_task,
    post_payment_method_domains_payment_method_domain_validate_builder, post_payment_method_domains_payment_method_domain_validate_task,
    post_payment_methods_builder, post_payment_methods_task,
    post_payment_methods_payment_method_builder, post_payment_methods_payment_method_task,
    post_payment_methods_payment_method_attach_builder, post_payment_methods_payment_method_attach_task,
    post_payment_methods_payment_method_detach_builder, post_payment_methods_payment_method_detach_task,
    post_payment_records_report_payment_builder, post_payment_records_report_payment_task,
    post_payment_records_id_report_payment_attempt_builder, post_payment_records_id_report_payment_attempt_task,
    post_payment_records_id_report_payment_attempt_canceled_builder, post_payment_records_id_report_payment_attempt_canceled_task,
    post_payment_records_id_report_payment_attempt_failed_builder, post_payment_records_id_report_payment_attempt_failed_task,
    post_payment_records_id_report_payment_attempt_guaranteed_builder, post_payment_records_id_report_payment_attempt_guaranteed_task,
    post_payment_records_id_report_payment_attempt_informational_builder, post_payment_records_id_report_payment_attempt_informational_task,
    post_payment_records_id_report_refund_builder, post_payment_records_id_report_refund_task,
    post_payouts_builder, post_payouts_task,
    post_payouts_payout_builder, post_payouts_payout_task,
    post_payouts_payout_cancel_builder, post_payouts_payout_cancel_task,
    post_payouts_payout_reverse_builder, post_payouts_payout_reverse_task,
    post_plans_builder, post_plans_task,
    post_plans_plan_builder, post_plans_plan_task,
    delete_plans_plan_builder, delete_plans_plan_task,
    post_prices_builder, post_prices_task,
    post_prices_price_builder, post_prices_price_task,
    post_products_builder, post_products_task,
    post_products_id_builder, post_products_id_task,
    delete_products_id_builder, delete_products_id_task,
    post_products_product_features_builder, post_products_product_features_task,
    delete_products_product_features_id_builder, delete_products_product_features_id_task,
    post_promotion_codes_builder, post_promotion_codes_task,
    post_promotion_codes_promotion_code_builder, post_promotion_codes_promotion_code_task,
    post_quotes_builder, post_quotes_task,
    post_quotes_quote_builder, post_quotes_quote_task,
    post_quotes_quote_accept_builder, post_quotes_quote_accept_task,
    post_quotes_quote_cancel_builder, post_quotes_quote_cancel_task,
    post_quotes_quote_finalize_builder, post_quotes_quote_finalize_task,
    post_radar_payment_evaluations_builder, post_radar_payment_evaluations_task,
    post_radar_value_list_items_builder, post_radar_value_list_items_task,
    delete_radar_value_list_items_item_builder, delete_radar_value_list_items_item_task,
    post_radar_value_lists_builder, post_radar_value_lists_task,
    post_radar_value_lists_value_list_builder, post_radar_value_lists_value_list_task,
    delete_radar_value_lists_value_list_builder, delete_radar_value_lists_value_list_task,
    post_refunds_builder, post_refunds_task,
    post_refunds_refund_builder, post_refunds_refund_task,
    post_refunds_refund_cancel_builder, post_refunds_refund_cancel_task,
    post_reporting_report_runs_builder, post_reporting_report_runs_task,
    post_reviews_review_approve_builder, post_reviews_review_approve_task,
    post_setup_intents_builder, post_setup_intents_task,
    post_setup_intents_intent_builder, post_setup_intents_intent_task,
    post_setup_intents_intent_cancel_builder, post_setup_intents_intent_cancel_task,
    post_setup_intents_intent_confirm_builder, post_setup_intents_intent_confirm_task,
    post_setup_intents_intent_verify_microdeposits_builder, post_setup_intents_intent_verify_microdeposits_task,
    post_shipping_rates_builder, post_shipping_rates_task,
    post_shipping_rates_shipping_rate_token_builder, post_shipping_rates_shipping_rate_token_task,
    post_sigma_saved_queries_id_builder, post_sigma_saved_queries_id_task,
    post_sources_builder, post_sources_task,
    post_sources_source_builder, post_sources_source_task,
    post_sources_source_verify_builder, post_sources_source_verify_task,
    post_subscription_items_builder, post_subscription_items_task,
    post_subscription_items_item_builder, post_subscription_items_item_task,
    delete_subscription_items_item_builder, delete_subscription_items_item_task,
    post_subscription_schedules_builder, post_subscription_schedules_task,
    post_subscription_schedules_schedule_builder, post_subscription_schedules_schedule_task,
    post_subscription_schedules_schedule_cancel_builder, post_subscription_schedules_schedule_cancel_task,
    post_subscription_schedules_schedule_release_builder, post_subscription_schedules_schedule_release_task,
    post_subscriptions_builder, post_subscriptions_task,
    post_subscriptions_subscription_exposed_id_builder, post_subscriptions_subscription_exposed_id_task,
    delete_subscriptions_subscription_exposed_id_builder, delete_subscriptions_subscription_exposed_id_task,
    delete_subscriptions_subscription_exposed_id_discount_builder, delete_subscriptions_subscription_exposed_id_discount_task,
    post_subscriptions_subscription_migrate_builder, post_subscriptions_subscription_migrate_task,
    post_subscriptions_subscription_resume_builder, post_subscriptions_subscription_resume_task,
    post_tax_calculations_builder, post_tax_calculations_task,
    post_tax_registrations_builder, post_tax_registrations_task,
    post_tax_registrations_id_builder, post_tax_registrations_id_task,
    post_tax_settings_builder, post_tax_settings_task,
    post_tax_transactions_create_from_calculation_builder, post_tax_transactions_create_from_calculation_task,
    post_tax_transactions_create_reversal_builder, post_tax_transactions_create_reversal_task,
    post_tax_ids_builder, post_tax_ids_task,
    delete_tax_ids_id_builder, delete_tax_ids_id_task,
    post_tax_rates_builder, post_tax_rates_task,
    post_tax_rates_tax_rate_builder, post_tax_rates_tax_rate_task,
    post_terminal_configurations_builder, post_terminal_configurations_task,
    post_terminal_configurations_configuration_builder, post_terminal_configurations_configuration_task,
    delete_terminal_configurations_configuration_builder, delete_terminal_configurations_configuration_task,
    post_terminal_connection_tokens_builder, post_terminal_connection_tokens_task,
    post_terminal_locations_builder, post_terminal_locations_task,
    post_terminal_locations_location_builder, post_terminal_locations_location_task,
    delete_terminal_locations_location_builder, delete_terminal_locations_location_task,
    post_terminal_onboarding_links_builder, post_terminal_onboarding_links_task,
    post_terminal_readers_builder, post_terminal_readers_task,
    post_terminal_readers_reader_builder, post_terminal_readers_reader_task,
    delete_terminal_readers_reader_builder, delete_terminal_readers_reader_task,
    post_terminal_readers_reader_cancel_action_builder, post_terminal_readers_reader_cancel_action_task,
    post_terminal_readers_reader_collect_inputs_builder, post_terminal_readers_reader_collect_inputs_task,
    post_terminal_readers_reader_collect_payment_method_builder, post_terminal_readers_reader_collect_payment_method_task,
    post_terminal_readers_reader_confirm_payment_intent_builder, post_terminal_readers_reader_confirm_payment_intent_task,
    post_terminal_readers_reader_process_payment_intent_builder, post_terminal_readers_reader_process_payment_intent_task,
    post_terminal_readers_reader_process_setup_intent_builder, post_terminal_readers_reader_process_setup_intent_task,
    post_terminal_readers_reader_refund_payment_builder, post_terminal_readers_reader_refund_payment_task,
    post_terminal_readers_reader_set_reader_display_builder, post_terminal_readers_reader_set_reader_display_task,
    post_terminal_refunds_builder, post_terminal_refunds_task,
    post_test_helpers_confirmation_tokens_builder, post_test_helpers_confirmation_tokens_task,
    post_test_helpers_customers_customer_fund_cash_balance_builder, post_test_helpers_customers_customer_fund_cash_balance_task,
    post_test_helpers_issuing_authorizations_builder, post_test_helpers_issuing_authorizations_task,
    post_test_helpers_issuing_authorizations_authorization_capture_builder, post_test_helpers_issuing_authorizations_authorization_capture_task,
    post_test_helpers_issuing_authorizations_authorization_expire_builder, post_test_helpers_issuing_authorizations_authorization_expire_task,
    post_test_helpers_issuing_authorizations_authorization_finalize_amount_builder, post_test_helpers_issuing_authorizations_authorization_finalize_amount_task,
    post_test_helpers_issuing_authorizations_authorization_fraud_challenges_respond_builder, post_test_helpers_issuing_authorizations_authorization_fraud_challenges_respond_task,
    post_test_helpers_issuing_authorizations_authorization_increment_builder, post_test_helpers_issuing_authorizations_authorization_increment_task,
    post_test_helpers_issuing_authorizations_authorization_reverse_builder, post_test_helpers_issuing_authorizations_authorization_reverse_task,
    post_test_helpers_issuing_cards_card_shipping_deliver_builder, post_test_helpers_issuing_cards_card_shipping_deliver_task,
    post_test_helpers_issuing_cards_card_shipping_fail_builder, post_test_helpers_issuing_cards_card_shipping_fail_task,
    post_test_helpers_issuing_cards_card_shipping_return_builder, post_test_helpers_issuing_cards_card_shipping_return_task,
    post_test_helpers_issuing_cards_card_shipping_ship_builder, post_test_helpers_issuing_cards_card_shipping_ship_task,
    post_test_helpers_issuing_cards_card_shipping_submit_builder, post_test_helpers_issuing_cards_card_shipping_submit_task,
    post_test_helpers_issuing_personalization_designs_personalization_design_activate_builder, post_test_helpers_issuing_personalization_designs_personalization_design_activate_task,
    post_test_helpers_issuing_personalization_designs_personalization_design_deactivate_builder, post_test_helpers_issuing_personalization_designs_personalization_design_deactivate_task,
    post_test_helpers_issuing_personalization_designs_personalization_design_reject_builder, post_test_helpers_issuing_personalization_designs_personalization_design_reject_task,
    post_test_helpers_issuing_settlements_builder, post_test_helpers_issuing_settlements_task,
    post_test_helpers_issuing_settlements_settlement_complete_builder, post_test_helpers_issuing_settlements_settlement_complete_task,
    post_test_helpers_issuing_transactions_create_force_capture_builder, post_test_helpers_issuing_transactions_create_force_capture_task,
    post_test_helpers_issuing_transactions_create_unlinked_refund_builder, post_test_helpers_issuing_transactions_create_unlinked_refund_task,
    post_test_helpers_issuing_transactions_transaction_refund_builder, post_test_helpers_issuing_transactions_transaction_refund_task,
    post_test_helpers_refunds_refund_expire_builder, post_test_helpers_refunds_refund_expire_task,
    post_test_helpers_terminal_readers_reader_present_payment_method_builder, post_test_helpers_terminal_readers_reader_present_payment_method_task,
    post_test_helpers_terminal_readers_reader_succeed_input_collection_builder, post_test_helpers_terminal_readers_reader_succeed_input_collection_task,
    post_test_helpers_terminal_readers_reader_timeout_input_collection_builder, post_test_helpers_terminal_readers_reader_timeout_input_collection_task,
    post_test_helpers_test_clocks_builder, post_test_helpers_test_clocks_task,
    delete_test_helpers_test_clocks_test_clock_builder, delete_test_helpers_test_clocks_test_clock_task,
    post_test_helpers_test_clocks_test_clock_advance_builder, post_test_helpers_test_clocks_test_clock_advance_task,
    post_test_helpers_treasury_inbound_transfers_id_fail_builder, post_test_helpers_treasury_inbound_transfers_id_fail_task,
    post_test_helpers_treasury_inbound_transfers_id_return_builder, post_test_helpers_treasury_inbound_transfers_id_return_task,
    post_test_helpers_treasury_inbound_transfers_id_succeed_builder, post_test_helpers_treasury_inbound_transfers_id_succeed_task,
    post_test_helpers_treasury_outbound_payments_id_builder, post_test_helpers_treasury_outbound_payments_id_task,
    post_test_helpers_treasury_outbound_payments_id_fail_builder, post_test_helpers_treasury_outbound_payments_id_fail_task,
    post_test_helpers_treasury_outbound_payments_id_post_builder, post_test_helpers_treasury_outbound_payments_id_post_task,
    post_test_helpers_treasury_outbound_payments_id_return_builder, post_test_helpers_treasury_outbound_payments_id_return_task,
    post_test_helpers_treasury_outbound_transfers_outbound_transfer_builder, post_test_helpers_treasury_outbound_transfers_outbound_transfer_task,
    post_test_helpers_treasury_outbound_transfers_outbound_transfer_fail_builder, post_test_helpers_treasury_outbound_transfers_outbound_transfer_fail_task,
    post_test_helpers_treasury_outbound_transfers_outbound_transfer_post_builder, post_test_helpers_treasury_outbound_transfers_outbound_transfer_post_task,
    post_test_helpers_treasury_outbound_transfers_outbound_transfer_return_builder, post_test_helpers_treasury_outbound_transfers_outbound_transfer_return_task,
    post_test_helpers_treasury_received_credits_builder, post_test_helpers_treasury_received_credits_task,
    post_test_helpers_treasury_received_debits_builder, post_test_helpers_treasury_received_debits_task,
    post_tokens_builder, post_tokens_task,
    post_topups_builder, post_topups_task,
    post_topups_topup_builder, post_topups_topup_task,
    post_topups_topup_cancel_builder, post_topups_topup_cancel_task,
    post_transfers_builder, post_transfers_task,
    post_transfers_id_reversals_builder, post_transfers_id_reversals_task,
    post_transfers_transfer_builder, post_transfers_transfer_task,
    post_transfers_transfer_reversals_id_builder, post_transfers_transfer_reversals_id_task,
    post_treasury_credit_reversals_builder, post_treasury_credit_reversals_task,
    post_treasury_debit_reversals_builder, post_treasury_debit_reversals_task,
    post_treasury_financial_accounts_builder, post_treasury_financial_accounts_task,
    post_treasury_financial_accounts_financial_account_builder, post_treasury_financial_accounts_financial_account_task,
    post_treasury_financial_accounts_financial_account_close_builder, post_treasury_financial_accounts_financial_account_close_task,
    post_treasury_financial_accounts_financial_account_features_builder, post_treasury_financial_accounts_financial_account_features_task,
    post_treasury_inbound_transfers_builder, post_treasury_inbound_transfers_task,
    post_treasury_inbound_transfers_inbound_transfer_cancel_builder, post_treasury_inbound_transfers_inbound_transfer_cancel_task,
    post_treasury_outbound_payments_builder, post_treasury_outbound_payments_task,
    post_treasury_outbound_payments_id_cancel_builder, post_treasury_outbound_payments_id_cancel_task,
    post_treasury_outbound_transfers_builder, post_treasury_outbound_transfers_task,
    post_treasury_outbound_transfers_outbound_transfer_cancel_builder, post_treasury_outbound_transfers_outbound_transfer_cancel_task,
    post_webhook_endpoints_builder, post_webhook_endpoints_task,
    post_webhook_endpoints_webhook_endpoint_builder, post_webhook_endpoints_webhook_endpoint_task,
    delete_webhook_endpoints_webhook_endpoint_builder, delete_webhook_endpoints_webhook_endpoint_task,
};
use crate::providers::stripe::clients::types::{ApiError, ApiPending};
use crate::providers::stripe::clients::stripe::Account;
use crate::providers::stripe::clients::stripe::AccountLink;
use crate::providers::stripe::clients::stripe::AccountSession;
use crate::providers::stripe::clients::stripe::ApplePayDomain;
use crate::providers::stripe::clients::stripe::ApplicationFee;
use crate::providers::stripe::clients::stripe::AppsSecret;
use crate::providers::stripe::clients::stripe::BalanceSettings;
use crate::providers::stripe::clients::stripe::BankAccount;
use crate::providers::stripe::clients::stripe::BillingAlert;
use crate::providers::stripe::clients::stripe::BillingCreditGrant;
use crate::providers::stripe::clients::stripe::BillingMeter;
use crate::providers::stripe::clients::stripe::BillingMeterEvent;
use crate::providers::stripe::clients::stripe::BillingMeterEventAdjustment;
use crate::providers::stripe::clients::stripe::BillingPortalConfiguration;
use crate::providers::stripe::clients::stripe::BillingPortalSession;
use crate::providers::stripe::clients::stripe::Capability;
use crate::providers::stripe::clients::stripe::CashBalance;
use crate::providers::stripe::clients::stripe::Charge;
use crate::providers::stripe::clients::stripe::CheckoutSession;
use crate::providers::stripe::clients::stripe::ClimateOrder;
use crate::providers::stripe::clients::stripe::ConfirmationToken;
use crate::providers::stripe::clients::stripe::Coupon;
use crate::providers::stripe::clients::stripe::CreditNote;
use crate::providers::stripe::clients::stripe::Customer;
use crate::providers::stripe::clients::stripe::CustomerBalanceTransaction;
use crate::providers::stripe::clients::stripe::CustomerCashBalanceTransaction;
use crate::providers::stripe::clients::stripe::CustomerSession;
use crate::providers::stripe::clients::stripe::DeletedAccount;
use crate::providers::stripe::clients::stripe::DeletedApplePayDomain;
use crate::providers::stripe::clients::stripe::DeletedCoupon;
use crate::providers::stripe::clients::stripe::DeletedCustomer;
use crate::providers::stripe::clients::stripe::DeletedDiscount;
use crate::providers::stripe::clients::stripe::DeletedInvoice;
use crate::providers::stripe::clients::stripe::DeletedInvoiceitem;
use crate::providers::stripe::clients::stripe::DeletedPerson;
use crate::providers::stripe::clients::stripe::DeletedPlan;
use crate::providers::stripe::clients::stripe::DeletedProduct;
use crate::providers::stripe::clients::stripe::DeletedProductFeature;
use crate::providers::stripe::clients::stripe::DeletedRadarValueList;
use crate::providers::stripe::clients::stripe::DeletedRadarValueListItem;
use crate::providers::stripe::clients::stripe::DeletedSubscriptionItem;
use crate::providers::stripe::clients::stripe::DeletedTaxId;
use crate::providers::stripe::clients::stripe::DeletedTerminalConfiguration;
use crate::providers::stripe::clients::stripe::DeletedTerminalLocation;
use crate::providers::stripe::clients::stripe::DeletedTerminalReader;
use crate::providers::stripe::clients::stripe::DeletedTestHelpersTestClock;
use crate::providers::stripe::clients::stripe::DeletedWebhookEndpoint;
use crate::providers::stripe::clients::stripe::Dispute;
use crate::providers::stripe::clients::stripe::EntitlementsFeature;
use crate::providers::stripe::clients::stripe::EphemeralKey;
use crate::providers::stripe::clients::stripe::FeeRefund;
use crate::providers::stripe::clients::stripe::File;
use crate::providers::stripe::clients::stripe::FileLink;
use crate::providers::stripe::clients::stripe::FinancialConnectionsAccount;
use crate::providers::stripe::clients::stripe::FinancialConnectionsSession;
use crate::providers::stripe::clients::stripe::ForwardingRequest;
use crate::providers::stripe::clients::stripe::FundingInstructions;
use crate::providers::stripe::clients::stripe::IdentityVerificationSession;
use crate::providers::stripe::clients::stripe::Invoice;
use crate::providers::stripe::clients::stripe::InvoiceRenderingTemplate;
use crate::providers::stripe::clients::stripe::Invoiceitem;
use crate::providers::stripe::clients::stripe::IssuingAuthorization;
use crate::providers::stripe::clients::stripe::IssuingCard;
use crate::providers::stripe::clients::stripe::IssuingCardholder;
use crate::providers::stripe::clients::stripe::IssuingDispute;
use crate::providers::stripe::clients::stripe::IssuingPersonalizationDesign;
use crate::providers::stripe::clients::stripe::IssuingSettlement;
use crate::providers::stripe::clients::stripe::IssuingToken;
use crate::providers::stripe::clients::stripe::IssuingTransaction;
use crate::providers::stripe::clients::stripe::LineItem;
use crate::providers::stripe::clients::stripe::LoginLink;
use crate::providers::stripe::clients::stripe::PaymentIntent;
use crate::providers::stripe::clients::stripe::PaymentLink;
use crate::providers::stripe::clients::stripe::PaymentMethod;
use crate::providers::stripe::clients::stripe::PaymentMethodConfiguration;
use crate::providers::stripe::clients::stripe::PaymentMethodDomain;
use crate::providers::stripe::clients::stripe::PaymentRecord;
use crate::providers::stripe::clients::stripe::Payout;
use crate::providers::stripe::clients::stripe::Person;
use crate::providers::stripe::clients::stripe::Plan;
use crate::providers::stripe::clients::stripe::Price;
use crate::providers::stripe::clients::stripe::Product;
use crate::providers::stripe::clients::stripe::ProductFeature;
use crate::providers::stripe::clients::stripe::PromotionCode;
use crate::providers::stripe::clients::stripe::Quote;
use crate::providers::stripe::clients::stripe::RadarPaymentEvaluation;
use crate::providers::stripe::clients::stripe::RadarValueList;
use crate::providers::stripe::clients::stripe::RadarValueListItem;
use crate::providers::stripe::clients::stripe::Refund;
use crate::providers::stripe::clients::stripe::ReportingReportRun;
use crate::providers::stripe::clients::stripe::Review;
use crate::providers::stripe::clients::stripe::SetupIntent;
use crate::providers::stripe::clients::stripe::ShippingRate;
use crate::providers::stripe::clients::stripe::SigmaSigmaApiQuery;
use crate::providers::stripe::clients::stripe::Source;
use crate::providers::stripe::clients::stripe::Subscription;
use crate::providers::stripe::clients::stripe::SubscriptionItem;
use crate::providers::stripe::clients::stripe::SubscriptionSchedule;
use crate::providers::stripe::clients::stripe::TaxCalculation;
use crate::providers::stripe::clients::stripe::TaxId;
use crate::providers::stripe::clients::stripe::TaxRate;
use crate::providers::stripe::clients::stripe::TaxRegistration;
use crate::providers::stripe::clients::stripe::TaxSettings;
use crate::providers::stripe::clients::stripe::TaxTransaction;
use crate::providers::stripe::clients::stripe::TerminalConfiguration;
use crate::providers::stripe::clients::stripe::TerminalConnectionToken;
use crate::providers::stripe::clients::stripe::TerminalLocation;
use crate::providers::stripe::clients::stripe::TerminalOnboardingLink;
use crate::providers::stripe::clients::stripe::TerminalReader;
use crate::providers::stripe::clients::stripe::TerminalRefund;
use crate::providers::stripe::clients::stripe::TestHelpersTestClock;
use crate::providers::stripe::clients::stripe::Token;
use crate::providers::stripe::clients::stripe::Topup;
use crate::providers::stripe::clients::stripe::Transfer;
use crate::providers::stripe::clients::stripe::TransferReversal;
use crate::providers::stripe::clients::stripe::TreasuryCreditReversal;
use crate::providers::stripe::clients::stripe::TreasuryDebitReversal;
use crate::providers::stripe::clients::stripe::TreasuryFinancialAccount;
use crate::providers::stripe::clients::stripe::TreasuryFinancialAccountFeatures;
use crate::providers::stripe::clients::stripe::TreasuryInboundTransfer;
use crate::providers::stripe::clients::stripe::TreasuryOutboundPayment;
use crate::providers::stripe::clients::stripe::TreasuryOutboundTransfer;
use crate::providers::stripe::clients::stripe::TreasuryReceivedCredit;
use crate::providers::stripe::clients::stripe::TreasuryReceivedDebit;
use crate::providers::stripe::clients::stripe::WebhookEndpoint;
use crate::providers::stripe::clients::stripe::DeleteAccountsAccountArgs;
use crate::providers::stripe::clients::stripe::DeleteAccountsAccountBankAccountsIdArgs;
use crate::providers::stripe::clients::stripe::DeleteAccountsAccountExternalAccountsIdArgs;
use crate::providers::stripe::clients::stripe::DeleteAccountsAccountPeoplePersonArgs;
use crate::providers::stripe::clients::stripe::DeleteAccountsAccountPersonsPersonArgs;
use crate::providers::stripe::clients::stripe::DeleteApplePayDomainsDomainArgs;
use crate::providers::stripe::clients::stripe::DeleteCouponsCouponArgs;
use crate::providers::stripe::clients::stripe::DeleteCustomersCustomerArgs;
use crate::providers::stripe::clients::stripe::DeleteCustomersCustomerBankAccountsIdArgs;
use crate::providers::stripe::clients::stripe::DeleteCustomersCustomerCardsIdArgs;
use crate::providers::stripe::clients::stripe::DeleteCustomersCustomerDiscountArgs;
use crate::providers::stripe::clients::stripe::DeleteCustomersCustomerSourcesIdArgs;
use crate::providers::stripe::clients::stripe::DeleteCustomersCustomerSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::stripe::DeleteCustomersCustomerSubscriptionsSubscriptionExposedIdDiscountArgs;
use crate::providers::stripe::clients::stripe::DeleteCustomersCustomerTaxIdsIdArgs;
use crate::providers::stripe::clients::stripe::DeleteEphemeralKeysKeyArgs;
use crate::providers::stripe::clients::stripe::DeleteInvoiceitemsInvoiceitemArgs;
use crate::providers::stripe::clients::stripe::DeleteInvoicesInvoiceArgs;
use crate::providers::stripe::clients::stripe::DeletePlansPlanArgs;
use crate::providers::stripe::clients::stripe::DeleteProductsIdArgs;
use crate::providers::stripe::clients::stripe::DeleteProductsProductFeaturesIdArgs;
use crate::providers::stripe::clients::stripe::DeleteRadarValueListItemsItemArgs;
use crate::providers::stripe::clients::stripe::DeleteRadarValueListsValueListArgs;
use crate::providers::stripe::clients::stripe::DeleteSubscriptionItemsItemArgs;
use crate::providers::stripe::clients::stripe::DeleteSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::stripe::DeleteSubscriptionsSubscriptionExposedIdDiscountArgs;
use crate::providers::stripe::clients::stripe::DeleteTaxIdsIdArgs;
use crate::providers::stripe::clients::stripe::DeleteTerminalConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::stripe::DeleteTerminalLocationsLocationArgs;
use crate::providers::stripe::clients::stripe::DeleteTerminalReadersReaderArgs;
use crate::providers::stripe::clients::stripe::DeleteTestHelpersTestClocksTestClockArgs;
use crate::providers::stripe::clients::stripe::DeleteWebhookEndpointsWebhookEndpointArgs;
use crate::providers::stripe::clients::stripe::PostAccountLinksArgs;
use crate::providers::stripe::clients::stripe::PostAccountSessionsArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountBankAccountsArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountBankAccountsIdArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountCapabilitiesCapabilityArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountExternalAccountsArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountExternalAccountsIdArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountLoginLinksArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountPeopleArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountPeoplePersonArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountPersonsArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountPersonsPersonArgs;
use crate::providers::stripe::clients::stripe::PostAccountsAccountRejectArgs;
use crate::providers::stripe::clients::stripe::PostAccountsArgs;
use crate::providers::stripe::clients::stripe::PostApplePayDomainsArgs;
use crate::providers::stripe::clients::stripe::PostApplicationFeesFeeRefundsIdArgs;
use crate::providers::stripe::clients::stripe::PostApplicationFeesIdRefundArgs;
use crate::providers::stripe::clients::stripe::PostApplicationFeesIdRefundsArgs;
use crate::providers::stripe::clients::stripe::PostAppsSecretsArgs;
use crate::providers::stripe::clients::stripe::PostAppsSecretsDeleteArgs;
use crate::providers::stripe::clients::stripe::PostBalanceSettingsArgs;
use crate::providers::stripe::clients::stripe::PostBillingAlertsArgs;
use crate::providers::stripe::clients::stripe::PostBillingAlertsIdActivateArgs;
use crate::providers::stripe::clients::stripe::PostBillingAlertsIdArchiveArgs;
use crate::providers::stripe::clients::stripe::PostBillingAlertsIdDeactivateArgs;
use crate::providers::stripe::clients::stripe::PostBillingCreditGrantsArgs;
use crate::providers::stripe::clients::stripe::PostBillingCreditGrantsIdArgs;
use crate::providers::stripe::clients::stripe::PostBillingCreditGrantsIdExpireArgs;
use crate::providers::stripe::clients::stripe::PostBillingCreditGrantsIdVoidArgs;
use crate::providers::stripe::clients::stripe::PostBillingMeterEventAdjustmentsArgs;
use crate::providers::stripe::clients::stripe::PostBillingMeterEventsArgs;
use crate::providers::stripe::clients::stripe::PostBillingMetersArgs;
use crate::providers::stripe::clients::stripe::PostBillingMetersIdArgs;
use crate::providers::stripe::clients::stripe::PostBillingMetersIdDeactivateArgs;
use crate::providers::stripe::clients::stripe::PostBillingMetersIdReactivateArgs;
use crate::providers::stripe::clients::stripe::PostBillingPortalConfigurationsArgs;
use crate::providers::stripe::clients::stripe::PostBillingPortalConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::stripe::PostBillingPortalSessionsArgs;
use crate::providers::stripe::clients::stripe::PostChargesArgs;
use crate::providers::stripe::clients::stripe::PostChargesChargeArgs;
use crate::providers::stripe::clients::stripe::PostChargesChargeCaptureArgs;
use crate::providers::stripe::clients::stripe::PostChargesChargeDisputeArgs;
use crate::providers::stripe::clients::stripe::PostChargesChargeDisputeCloseArgs;
use crate::providers::stripe::clients::stripe::PostChargesChargeRefundArgs;
use crate::providers::stripe::clients::stripe::PostChargesChargeRefundsArgs;
use crate::providers::stripe::clients::stripe::PostChargesChargeRefundsRefundArgs;
use crate::providers::stripe::clients::stripe::PostCheckoutSessionsArgs;
use crate::providers::stripe::clients::stripe::PostCheckoutSessionsSessionArgs;
use crate::providers::stripe::clients::stripe::PostCheckoutSessionsSessionExpireArgs;
use crate::providers::stripe::clients::stripe::PostClimateOrdersArgs;
use crate::providers::stripe::clients::stripe::PostClimateOrdersOrderArgs;
use crate::providers::stripe::clients::stripe::PostClimateOrdersOrderCancelArgs;
use crate::providers::stripe::clients::stripe::PostCouponsArgs;
use crate::providers::stripe::clients::stripe::PostCouponsCouponArgs;
use crate::providers::stripe::clients::stripe::PostCreditNotesArgs;
use crate::providers::stripe::clients::stripe::PostCreditNotesIdArgs;
use crate::providers::stripe::clients::stripe::PostCreditNotesIdVoidArgs;
use crate::providers::stripe::clients::stripe::PostCustomerSessionsArgs;
use crate::providers::stripe::clients::stripe::PostCustomersArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerBalanceTransactionsArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerBalanceTransactionsTransactionArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerBankAccountsArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerBankAccountsIdArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerBankAccountsIdVerifyArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerCardsArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerCardsIdArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerCashBalanceArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerFundingInstructionsArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerSourcesArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerSourcesIdArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerSourcesIdVerifyArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerSubscriptionsArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::stripe::PostCustomersCustomerTaxIdsArgs;
use crate::providers::stripe::clients::stripe::PostDisputesDisputeArgs;
use crate::providers::stripe::clients::stripe::PostDisputesDisputeCloseArgs;
use crate::providers::stripe::clients::stripe::PostEntitlementsFeaturesArgs;
use crate::providers::stripe::clients::stripe::PostEntitlementsFeaturesIdArgs;
use crate::providers::stripe::clients::stripe::PostEphemeralKeysArgs;
use crate::providers::stripe::clients::stripe::PostExternalAccountsIdArgs;
use crate::providers::stripe::clients::stripe::PostFileLinksArgs;
use crate::providers::stripe::clients::stripe::PostFileLinksLinkArgs;
use crate::providers::stripe::clients::stripe::PostFilesArgs;
use crate::providers::stripe::clients::stripe::PostFinancialConnectionsAccountsAccountDisconnectArgs;
use crate::providers::stripe::clients::stripe::PostFinancialConnectionsAccountsAccountRefreshArgs;
use crate::providers::stripe::clients::stripe::PostFinancialConnectionsAccountsAccountSubscribeArgs;
use crate::providers::stripe::clients::stripe::PostFinancialConnectionsAccountsAccountUnsubscribeArgs;
use crate::providers::stripe::clients::stripe::PostFinancialConnectionsSessionsArgs;
use crate::providers::stripe::clients::stripe::PostForwardingRequestsArgs;
use crate::providers::stripe::clients::stripe::PostIdentityVerificationSessionsArgs;
use crate::providers::stripe::clients::stripe::PostIdentityVerificationSessionsSessionArgs;
use crate::providers::stripe::clients::stripe::PostIdentityVerificationSessionsSessionCancelArgs;
use crate::providers::stripe::clients::stripe::PostIdentityVerificationSessionsSessionRedactArgs;
use crate::providers::stripe::clients::stripe::PostInvoiceRenderingTemplatesTemplateArchiveArgs;
use crate::providers::stripe::clients::stripe::PostInvoiceRenderingTemplatesTemplateUnarchiveArgs;
use crate::providers::stripe::clients::stripe::PostInvoiceitemsArgs;
use crate::providers::stripe::clients::stripe::PostInvoiceitemsInvoiceitemArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesCreatePreviewArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceAddLinesArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceAttachPaymentArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceFinalizeArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceLinesLineItemIdArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceMarkUncollectibleArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoicePayArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceRemoveLinesArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceSendArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceUpdateLinesArgs;
use crate::providers::stripe::clients::stripe::PostInvoicesInvoiceVoidArgs;
use crate::providers::stripe::clients::stripe::PostIssuingAuthorizationsAuthorizationApproveArgs;
use crate::providers::stripe::clients::stripe::PostIssuingAuthorizationsAuthorizationArgs;
use crate::providers::stripe::clients::stripe::PostIssuingAuthorizationsAuthorizationDeclineArgs;
use crate::providers::stripe::clients::stripe::PostIssuingCardholdersArgs;
use crate::providers::stripe::clients::stripe::PostIssuingCardholdersCardholderArgs;
use crate::providers::stripe::clients::stripe::PostIssuingCardsArgs;
use crate::providers::stripe::clients::stripe::PostIssuingCardsCardArgs;
use crate::providers::stripe::clients::stripe::PostIssuingDisputesArgs;
use crate::providers::stripe::clients::stripe::PostIssuingDisputesDisputeArgs;
use crate::providers::stripe::clients::stripe::PostIssuingDisputesDisputeSubmitArgs;
use crate::providers::stripe::clients::stripe::PostIssuingPersonalizationDesignsArgs;
use crate::providers::stripe::clients::stripe::PostIssuingPersonalizationDesignsPersonalizationDesignArgs;
use crate::providers::stripe::clients::stripe::PostIssuingSettlementsSettlementArgs;
use crate::providers::stripe::clients::stripe::PostIssuingTokensTokenArgs;
use crate::providers::stripe::clients::stripe::PostIssuingTransactionsTransactionArgs;
use crate::providers::stripe::clients::stripe::PostLinkAccountSessionsArgs;
use crate::providers::stripe::clients::stripe::PostLinkedAccountsAccountDisconnectArgs;
use crate::providers::stripe::clients::stripe::PostLinkedAccountsAccountRefreshArgs;
use crate::providers::stripe::clients::stripe::PostPaymentIntentsArgs;
use crate::providers::stripe::clients::stripe::PostPaymentIntentsIntentApplyCustomerBalanceArgs;
use crate::providers::stripe::clients::stripe::PostPaymentIntentsIntentArgs;
use crate::providers::stripe::clients::stripe::PostPaymentIntentsIntentCancelArgs;
use crate::providers::stripe::clients::stripe::PostPaymentIntentsIntentCaptureArgs;
use crate::providers::stripe::clients::stripe::PostPaymentIntentsIntentConfirmArgs;
use crate::providers::stripe::clients::stripe::PostPaymentIntentsIntentIncrementAuthorizationArgs;
use crate::providers::stripe::clients::stripe::PostPaymentIntentsIntentVerifyMicrodepositsArgs;
use crate::providers::stripe::clients::stripe::PostPaymentLinksArgs;
use crate::providers::stripe::clients::stripe::PostPaymentLinksPaymentLinkArgs;
use crate::providers::stripe::clients::stripe::PostPaymentMethodConfigurationsArgs;
use crate::providers::stripe::clients::stripe::PostPaymentMethodConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::stripe::PostPaymentMethodDomainsArgs;
use crate::providers::stripe::clients::stripe::PostPaymentMethodDomainsPaymentMethodDomainArgs;
use crate::providers::stripe::clients::stripe::PostPaymentMethodDomainsPaymentMethodDomainValidateArgs;
use crate::providers::stripe::clients::stripe::PostPaymentMethodsArgs;
use crate::providers::stripe::clients::stripe::PostPaymentMethodsPaymentMethodArgs;
use crate::providers::stripe::clients::stripe::PostPaymentMethodsPaymentMethodAttachArgs;
use crate::providers::stripe::clients::stripe::PostPaymentMethodsPaymentMethodDetachArgs;
use crate::providers::stripe::clients::stripe::PostPaymentRecordsIdReportPaymentAttemptArgs;
use crate::providers::stripe::clients::stripe::PostPaymentRecordsIdReportPaymentAttemptCanceledArgs;
use crate::providers::stripe::clients::stripe::PostPaymentRecordsIdReportPaymentAttemptFailedArgs;
use crate::providers::stripe::clients::stripe::PostPaymentRecordsIdReportPaymentAttemptGuaranteedArgs;
use crate::providers::stripe::clients::stripe::PostPaymentRecordsIdReportPaymentAttemptInformationalArgs;
use crate::providers::stripe::clients::stripe::PostPaymentRecordsIdReportRefundArgs;
use crate::providers::stripe::clients::stripe::PostPaymentRecordsReportPaymentArgs;
use crate::providers::stripe::clients::stripe::PostPayoutsArgs;
use crate::providers::stripe::clients::stripe::PostPayoutsPayoutArgs;
use crate::providers::stripe::clients::stripe::PostPayoutsPayoutCancelArgs;
use crate::providers::stripe::clients::stripe::PostPayoutsPayoutReverseArgs;
use crate::providers::stripe::clients::stripe::PostPlansArgs;
use crate::providers::stripe::clients::stripe::PostPlansPlanArgs;
use crate::providers::stripe::clients::stripe::PostPricesArgs;
use crate::providers::stripe::clients::stripe::PostPricesPriceArgs;
use crate::providers::stripe::clients::stripe::PostProductsArgs;
use crate::providers::stripe::clients::stripe::PostProductsIdArgs;
use crate::providers::stripe::clients::stripe::PostProductsProductFeaturesArgs;
use crate::providers::stripe::clients::stripe::PostPromotionCodesArgs;
use crate::providers::stripe::clients::stripe::PostPromotionCodesPromotionCodeArgs;
use crate::providers::stripe::clients::stripe::PostQuotesArgs;
use crate::providers::stripe::clients::stripe::PostQuotesQuoteAcceptArgs;
use crate::providers::stripe::clients::stripe::PostQuotesQuoteArgs;
use crate::providers::stripe::clients::stripe::PostQuotesQuoteCancelArgs;
use crate::providers::stripe::clients::stripe::PostQuotesQuoteFinalizeArgs;
use crate::providers::stripe::clients::stripe::PostRadarPaymentEvaluationsArgs;
use crate::providers::stripe::clients::stripe::PostRadarValueListItemsArgs;
use crate::providers::stripe::clients::stripe::PostRadarValueListsArgs;
use crate::providers::stripe::clients::stripe::PostRadarValueListsValueListArgs;
use crate::providers::stripe::clients::stripe::PostRefundsArgs;
use crate::providers::stripe::clients::stripe::PostRefundsRefundArgs;
use crate::providers::stripe::clients::stripe::PostRefundsRefundCancelArgs;
use crate::providers::stripe::clients::stripe::PostReportingReportRunsArgs;
use crate::providers::stripe::clients::stripe::PostReviewsReviewApproveArgs;
use crate::providers::stripe::clients::stripe::PostSetupIntentsArgs;
use crate::providers::stripe::clients::stripe::PostSetupIntentsIntentArgs;
use crate::providers::stripe::clients::stripe::PostSetupIntentsIntentCancelArgs;
use crate::providers::stripe::clients::stripe::PostSetupIntentsIntentConfirmArgs;
use crate::providers::stripe::clients::stripe::PostSetupIntentsIntentVerifyMicrodepositsArgs;
use crate::providers::stripe::clients::stripe::PostShippingRatesArgs;
use crate::providers::stripe::clients::stripe::PostShippingRatesShippingRateTokenArgs;
use crate::providers::stripe::clients::stripe::PostSigmaSavedQueriesIdArgs;
use crate::providers::stripe::clients::stripe::PostSourcesArgs;
use crate::providers::stripe::clients::stripe::PostSourcesSourceArgs;
use crate::providers::stripe::clients::stripe::PostSourcesSourceVerifyArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionItemsArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionItemsItemArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionSchedulesArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionSchedulesScheduleArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionSchedulesScheduleCancelArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionSchedulesScheduleReleaseArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionsArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionsSubscriptionMigrateArgs;
use crate::providers::stripe::clients::stripe::PostSubscriptionsSubscriptionResumeArgs;
use crate::providers::stripe::clients::stripe::PostTaxCalculationsArgs;
use crate::providers::stripe::clients::stripe::PostTaxIdsArgs;
use crate::providers::stripe::clients::stripe::PostTaxRatesArgs;
use crate::providers::stripe::clients::stripe::PostTaxRatesTaxRateArgs;
use crate::providers::stripe::clients::stripe::PostTaxRegistrationsArgs;
use crate::providers::stripe::clients::stripe::PostTaxRegistrationsIdArgs;
use crate::providers::stripe::clients::stripe::PostTaxSettingsArgs;
use crate::providers::stripe::clients::stripe::PostTaxTransactionsCreateFromCalculationArgs;
use crate::providers::stripe::clients::stripe::PostTaxTransactionsCreateReversalArgs;
use crate::providers::stripe::clients::stripe::PostTerminalConfigurationsArgs;
use crate::providers::stripe::clients::stripe::PostTerminalConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::stripe::PostTerminalConnectionTokensArgs;
use crate::providers::stripe::clients::stripe::PostTerminalLocationsArgs;
use crate::providers::stripe::clients::stripe::PostTerminalLocationsLocationArgs;
use crate::providers::stripe::clients::stripe::PostTerminalOnboardingLinksArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersReaderArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersReaderCancelActionArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersReaderCollectInputsArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersReaderCollectPaymentMethodArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersReaderConfirmPaymentIntentArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersReaderProcessPaymentIntentArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersReaderProcessSetupIntentArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersReaderRefundPaymentArgs;
use crate::providers::stripe::clients::stripe::PostTerminalReadersReaderSetReaderDisplayArgs;
use crate::providers::stripe::clients::stripe::PostTerminalRefundsArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersConfirmationTokensArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersCustomersCustomerFundCashBalanceArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingAuthorizationsArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingAuthorizationsAuthorizationCaptureArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingAuthorizationsAuthorizationExpireArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingAuthorizationsAuthorizationFinalizeAmountArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingAuthorizationsAuthorizationFraudChallengesRespondArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingAuthorizationsAuthorizationIncrementArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingAuthorizationsAuthorizationReverseArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingCardsCardShippingDeliverArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingCardsCardShippingFailArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingCardsCardShippingReturnArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingCardsCardShippingShipArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingCardsCardShippingSubmitArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingPersonalizationDesignsPersonalizationDesignActivateArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingPersonalizationDesignsPersonalizationDesignDeactivateArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingPersonalizationDesignsPersonalizationDesignRejectArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingSettlementsArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingSettlementsSettlementCompleteArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingTransactionsCreateForceCaptureArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingTransactionsCreateUnlinkedRefundArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersIssuingTransactionsTransactionRefundArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersRefundsRefundExpireArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTerminalReadersReaderPresentPaymentMethodArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTerminalReadersReaderSucceedInputCollectionArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTerminalReadersReaderTimeoutInputCollectionArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTestClocksArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTestClocksTestClockAdvanceArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryInboundTransfersIdFailArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryInboundTransfersIdReturnArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryInboundTransfersIdSucceedArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryOutboundPaymentsIdArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryOutboundPaymentsIdFailArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryOutboundPaymentsIdPostArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryOutboundPaymentsIdReturnArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryOutboundTransfersOutboundTransferArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryOutboundTransfersOutboundTransferFailArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryOutboundTransfersOutboundTransferPostArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryOutboundTransfersOutboundTransferReturnArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryReceivedCreditsArgs;
use crate::providers::stripe::clients::stripe::PostTestHelpersTreasuryReceivedDebitsArgs;
use crate::providers::stripe::clients::stripe::PostTokensArgs;
use crate::providers::stripe::clients::stripe::PostTopupsArgs;
use crate::providers::stripe::clients::stripe::PostTopupsTopupArgs;
use crate::providers::stripe::clients::stripe::PostTopupsTopupCancelArgs;
use crate::providers::stripe::clients::stripe::PostTransfersArgs;
use crate::providers::stripe::clients::stripe::PostTransfersIdReversalsArgs;
use crate::providers::stripe::clients::stripe::PostTransfersTransferArgs;
use crate::providers::stripe::clients::stripe::PostTransfersTransferReversalsIdArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryCreditReversalsArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryDebitReversalsArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryFinancialAccountsArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryFinancialAccountsFinancialAccountArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryFinancialAccountsFinancialAccountCloseArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryFinancialAccountsFinancialAccountFeaturesArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryInboundTransfersArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryInboundTransfersInboundTransferCancelArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryOutboundPaymentsArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryOutboundPaymentsIdCancelArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryOutboundTransfersArgs;
use crate::providers::stripe::clients::stripe::PostTreasuryOutboundTransfersOutboundTransferCancelArgs;
use crate::providers::stripe::clients::stripe::PostWebhookEndpointsArgs;
use crate::providers::stripe::clients::stripe::PostWebhookEndpointsWebhookEndpointArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// StripeProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let client = ProviderClient::new("my-project", "dev", state_store);
/// let http_client = SimpleHttpClient::new(...);
/// let provider = StripeProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct StripeProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> StripeProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new StripeProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Post account links.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_account_links(
        &self,
        args: &PostAccountLinksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountLink, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_account_links_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_account_links_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post account sessions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_account_sessions(
        &self,
        args: &PostAccountSessionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_account_sessions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_account_sessions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts(
        &self,
        args: &PostAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account(
        &self,
        args: &PostAccountsAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete accounts account.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_accounts_account(
        &self,
        args: &DeleteAccountsAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_accounts_account_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_accounts_account_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account bank accounts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_bank_accounts(
        &self,
        args: &PostAccountsAccountBankAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_bank_accounts_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_bank_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account bank accounts id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_bank_accounts_id(
        &self,
        args: &PostAccountsAccountBankAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_bank_accounts_id_builder(
            &self.http_client,
            &args.account,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_bank_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete accounts account bank accounts id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_accounts_account_bank_accounts_id(
        &self,
        args: &DeleteAccountsAccountBankAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_accounts_account_bank_accounts_id_builder(
            &self.http_client,
            &args.account,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_accounts_account_bank_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account capabilities capability.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Capability result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_capabilities_capability(
        &self,
        args: &PostAccountsAccountCapabilitiesCapabilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Capability, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_capabilities_capability_builder(
            &self.http_client,
            &args.account,
            &args.capability,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_capabilities_capability_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account external accounts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_external_accounts(
        &self,
        args: &PostAccountsAccountExternalAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_external_accounts_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_external_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account external accounts id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_external_accounts_id(
        &self,
        args: &PostAccountsAccountExternalAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_external_accounts_id_builder(
            &self.http_client,
            &args.account,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_external_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete accounts account external accounts id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_accounts_account_external_accounts_id(
        &self,
        args: &DeleteAccountsAccountExternalAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_accounts_account_external_accounts_id_builder(
            &self.http_client,
            &args.account,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_accounts_account_external_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account login links.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoginLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_login_links(
        &self,
        args: &PostAccountsAccountLoginLinksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoginLink, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_login_links_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_login_links_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account people.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Person result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_people(
        &self,
        args: &PostAccountsAccountPeopleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_people_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_people_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account people person.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Person result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_people_person(
        &self,
        args: &PostAccountsAccountPeoplePersonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_people_person_builder(
            &self.http_client,
            &args.account,
            &args.person,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_people_person_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete accounts account people person.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedPerson result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_accounts_account_people_person(
        &self,
        args: &DeleteAccountsAccountPeoplePersonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedPerson, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_accounts_account_people_person_builder(
            &self.http_client,
            &args.account,
            &args.person,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_accounts_account_people_person_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account persons.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Person result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_persons(
        &self,
        args: &PostAccountsAccountPersonsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_persons_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_persons_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account persons person.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Person result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_persons_person(
        &self,
        args: &PostAccountsAccountPersonsPersonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_persons_person_builder(
            &self.http_client,
            &args.account,
            &args.person,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_persons_person_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete accounts account persons person.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedPerson result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_accounts_account_persons_person(
        &self,
        args: &DeleteAccountsAccountPersonsPersonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedPerson, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_accounts_account_persons_person_builder(
            &self.http_client,
            &args.account,
            &args.person,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_accounts_account_persons_person_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post accounts account reject.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_reject(
        &self,
        args: &PostAccountsAccountRejectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_accounts_account_reject_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_accounts_account_reject_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post apple pay domains.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApplePayDomain result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_apple_pay_domains(
        &self,
        args: &PostApplePayDomainsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApplePayDomain, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_apple_pay_domains_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_apple_pay_domains_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete apple pay domains domain.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedApplePayDomain result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_apple_pay_domains_domain(
        &self,
        args: &DeleteApplePayDomainsDomainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedApplePayDomain, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_apple_pay_domains_domain_builder(
            &self.http_client,
            &args.domain,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_apple_pay_domains_domain_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post application fees fee refunds id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FeeRefund result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_application_fees_fee_refunds_id(
        &self,
        args: &PostApplicationFeesFeeRefundsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FeeRefund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_application_fees_fee_refunds_id_builder(
            &self.http_client,
            &args.fee,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_application_fees_fee_refunds_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post application fees id refund.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApplicationFee result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_application_fees_id_refund(
        &self,
        args: &PostApplicationFeesIdRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApplicationFee, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_application_fees_id_refund_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_application_fees_id_refund_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post application fees id refunds.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FeeRefund result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_application_fees_id_refunds(
        &self,
        args: &PostApplicationFeesIdRefundsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FeeRefund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_application_fees_id_refunds_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_application_fees_id_refunds_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post apps secrets.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppsSecret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_apps_secrets(
        &self,
        args: &PostAppsSecretsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppsSecret, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_apps_secrets_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_apps_secrets_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post apps secrets delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppsSecret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_apps_secrets_delete(
        &self,
        args: &PostAppsSecretsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppsSecret, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_apps_secrets_delete_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_apps_secrets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post balance settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BalanceSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_balance_settings(
        &self,
        args: &PostBalanceSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BalanceSettings, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_balance_settings_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_balance_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing alerts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAlert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_alerts(
        &self,
        args: &PostBillingAlertsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAlert, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_alerts_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_alerts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing alerts id activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAlert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_alerts_id_activate(
        &self,
        args: &PostBillingAlertsIdActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAlert, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_alerts_id_activate_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_alerts_id_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing alerts id archive.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAlert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_alerts_id_archive(
        &self,
        args: &PostBillingAlertsIdArchiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAlert, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_alerts_id_archive_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_alerts_id_archive_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing alerts id deactivate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAlert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_alerts_id_deactivate(
        &self,
        args: &PostBillingAlertsIdDeactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAlert, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_alerts_id_deactivate_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_alerts_id_deactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing credit grants.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingCreditGrant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_credit_grants(
        &self,
        args: &PostBillingCreditGrantsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingCreditGrant, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_credit_grants_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_credit_grants_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing credit grants id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingCreditGrant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_credit_grants_id(
        &self,
        args: &PostBillingCreditGrantsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingCreditGrant, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_credit_grants_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_credit_grants_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing credit grants id expire.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingCreditGrant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_credit_grants_id_expire(
        &self,
        args: &PostBillingCreditGrantsIdExpireArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingCreditGrant, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_credit_grants_id_expire_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_credit_grants_id_expire_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing credit grants id void.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingCreditGrant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_credit_grants_id_void(
        &self,
        args: &PostBillingCreditGrantsIdVoidArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingCreditGrant, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_credit_grants_id_void_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_credit_grants_id_void_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing meter event adjustments.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingMeterEventAdjustment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_meter_event_adjustments(
        &self,
        args: &PostBillingMeterEventAdjustmentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingMeterEventAdjustment, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_meter_event_adjustments_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_meter_event_adjustments_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing meter events.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingMeterEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_meter_events(
        &self,
        args: &PostBillingMeterEventsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingMeterEvent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_meter_events_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_meter_events_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing meters.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingMeter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_meters(
        &self,
        args: &PostBillingMetersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingMeter, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_meters_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_meters_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing meters id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingMeter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_meters_id(
        &self,
        args: &PostBillingMetersIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingMeter, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_meters_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_meters_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing meters id deactivate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingMeter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_meters_id_deactivate(
        &self,
        args: &PostBillingMetersIdDeactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingMeter, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_meters_id_deactivate_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_meters_id_deactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing meters id reactivate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingMeter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_meters_id_reactivate(
        &self,
        args: &PostBillingMetersIdReactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingMeter, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_meters_id_reactivate_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_meters_id_reactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing portal configurations.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingPortalConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_portal_configurations(
        &self,
        args: &PostBillingPortalConfigurationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingPortalConfiguration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_portal_configurations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_portal_configurations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing portal configurations configuration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingPortalConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_portal_configurations_configuration(
        &self,
        args: &PostBillingPortalConfigurationsConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingPortalConfiguration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_portal_configurations_configuration_builder(
            &self.http_client,
            &args.configuration,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_portal_configurations_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post billing portal sessions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingPortalSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_billing_portal_sessions(
        &self,
        args: &PostBillingPortalSessionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingPortalSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_billing_portal_sessions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_billing_portal_sessions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post charges.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Charge result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_charges(
        &self,
        args: &PostChargesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Charge, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_charges_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_charges_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post charges charge.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Charge result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_charges_charge(
        &self,
        args: &PostChargesChargeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Charge, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_charges_charge_builder(
            &self.http_client,
            &args.charge,
        )
        .map_err(ProviderError::Api)?;

        let task = post_charges_charge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post charges charge capture.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Charge result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_charges_charge_capture(
        &self,
        args: &PostChargesChargeCaptureArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Charge, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_charges_charge_capture_builder(
            &self.http_client,
            &args.charge,
        )
        .map_err(ProviderError::Api)?;

        let task = post_charges_charge_capture_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post charges charge dispute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dispute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_charges_charge_dispute(
        &self,
        args: &PostChargesChargeDisputeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_charges_charge_dispute_builder(
            &self.http_client,
            &args.charge,
        )
        .map_err(ProviderError::Api)?;

        let task = post_charges_charge_dispute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post charges charge dispute close.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dispute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_charges_charge_dispute_close(
        &self,
        args: &PostChargesChargeDisputeCloseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_charges_charge_dispute_close_builder(
            &self.http_client,
            &args.charge,
        )
        .map_err(ProviderError::Api)?;

        let task = post_charges_charge_dispute_close_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post charges charge refund.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Charge result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_charges_charge_refund(
        &self,
        args: &PostChargesChargeRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Charge, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_charges_charge_refund_builder(
            &self.http_client,
            &args.charge,
        )
        .map_err(ProviderError::Api)?;

        let task = post_charges_charge_refund_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post charges charge refunds.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Refund result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_charges_charge_refunds(
        &self,
        args: &PostChargesChargeRefundsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Refund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_charges_charge_refunds_builder(
            &self.http_client,
            &args.charge,
        )
        .map_err(ProviderError::Api)?;

        let task = post_charges_charge_refunds_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post charges charge refunds refund.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Refund result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_charges_charge_refunds_refund(
        &self,
        args: &PostChargesChargeRefundsRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Refund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_charges_charge_refunds_refund_builder(
            &self.http_client,
            &args.charge,
            &args.refund,
        )
        .map_err(ProviderError::Api)?;

        let task = post_charges_charge_refunds_refund_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post checkout sessions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckoutSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_checkout_sessions(
        &self,
        args: &PostCheckoutSessionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckoutSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_checkout_sessions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_checkout_sessions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post checkout sessions session.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckoutSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_checkout_sessions_session(
        &self,
        args: &PostCheckoutSessionsSessionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckoutSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_checkout_sessions_session_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = post_checkout_sessions_session_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post checkout sessions session expire.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckoutSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_checkout_sessions_session_expire(
        &self,
        args: &PostCheckoutSessionsSessionExpireArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckoutSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_checkout_sessions_session_expire_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = post_checkout_sessions_session_expire_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post climate orders.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClimateOrder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_climate_orders(
        &self,
        args: &PostClimateOrdersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClimateOrder, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_climate_orders_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_climate_orders_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post climate orders order.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClimateOrder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_climate_orders_order(
        &self,
        args: &PostClimateOrdersOrderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClimateOrder, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_climate_orders_order_builder(
            &self.http_client,
            &args.order,
        )
        .map_err(ProviderError::Api)?;

        let task = post_climate_orders_order_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post climate orders order cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClimateOrder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_climate_orders_order_cancel(
        &self,
        args: &PostClimateOrdersOrderCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClimateOrder, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_climate_orders_order_cancel_builder(
            &self.http_client,
            &args.order,
        )
        .map_err(ProviderError::Api)?;

        let task = post_climate_orders_order_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post coupons.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Coupon result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_coupons(
        &self,
        args: &PostCouponsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Coupon, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_coupons_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_coupons_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post coupons coupon.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Coupon result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_coupons_coupon(
        &self,
        args: &PostCouponsCouponArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Coupon, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_coupons_coupon_builder(
            &self.http_client,
            &args.coupon,
        )
        .map_err(ProviderError::Api)?;

        let task = post_coupons_coupon_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete coupons coupon.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedCoupon result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_coupons_coupon(
        &self,
        args: &DeleteCouponsCouponArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedCoupon, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_coupons_coupon_builder(
            &self.http_client,
            &args.coupon,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_coupons_coupon_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post credit notes.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreditNote result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_credit_notes(
        &self,
        args: &PostCreditNotesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreditNote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_credit_notes_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_credit_notes_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post credit notes id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreditNote result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_credit_notes_id(
        &self,
        args: &PostCreditNotesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreditNote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_credit_notes_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_credit_notes_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post credit notes id void.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreditNote result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_credit_notes_id_void(
        &self,
        args: &PostCreditNotesIdVoidArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreditNote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_credit_notes_id_void_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_credit_notes_id_void_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customer sessions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customer_sessions(
        &self,
        args: &PostCustomerSessionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customer_sessions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customer_sessions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers(
        &self,
        args: &PostCustomersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Customer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer(
        &self,
        args: &PostCustomersCustomerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Customer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete customers customer.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedCustomer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_customers_customer(
        &self,
        args: &DeleteCustomersCustomerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedCustomer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_customers_customer_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_customers_customer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer balance transactions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerBalanceTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_balance_transactions(
        &self,
        args: &PostCustomersCustomerBalanceTransactionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerBalanceTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_balance_transactions_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_balance_transactions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer balance transactions transaction.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerBalanceTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_balance_transactions_transaction(
        &self,
        args: &PostCustomersCustomerBalanceTransactionsTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerBalanceTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_balance_transactions_transaction_builder(
            &self.http_client,
            &args.customer,
            &args.transaction,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_balance_transactions_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer bank accounts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_bank_accounts(
        &self,
        args: &PostCustomersCustomerBankAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_bank_accounts_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_bank_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer bank accounts id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_bank_accounts_id(
        &self,
        args: &PostCustomersCustomerBankAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_bank_accounts_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_bank_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete customers customer bank accounts id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_customers_customer_bank_accounts_id(
        &self,
        args: &DeleteCustomersCustomerBankAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_customers_customer_bank_accounts_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_customers_customer_bank_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer bank accounts id verify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BankAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_bank_accounts_id_verify(
        &self,
        args: &PostCustomersCustomerBankAccountsIdVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BankAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_bank_accounts_id_verify_builder(
            &self.http_client,
            &args.customer,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_bank_accounts_id_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer cards.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_cards(
        &self,
        args: &PostCustomersCustomerCardsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_cards_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_cards_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer cards id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_cards_id(
        &self,
        args: &PostCustomersCustomerCardsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_cards_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_cards_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete customers customer cards id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_customers_customer_cards_id(
        &self,
        args: &DeleteCustomersCustomerCardsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_customers_customer_cards_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_customers_customer_cards_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer cash balance.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CashBalance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_cash_balance(
        &self,
        args: &PostCustomersCustomerCashBalanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CashBalance, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_cash_balance_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_cash_balance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete customers customer discount.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedDiscount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_customers_customer_discount(
        &self,
        args: &DeleteCustomersCustomerDiscountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedDiscount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_customers_customer_discount_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_customers_customer_discount_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer funding instructions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FundingInstructions result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_funding_instructions(
        &self,
        args: &PostCustomersCustomerFundingInstructionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FundingInstructions, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_funding_instructions_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_funding_instructions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer sources.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_sources(
        &self,
        args: &PostCustomersCustomerSourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_sources_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_sources_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer sources id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_sources_id(
        &self,
        args: &PostCustomersCustomerSourcesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_sources_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_sources_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete customers customer sources id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_customers_customer_sources_id(
        &self,
        args: &DeleteCustomersCustomerSourcesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_customers_customer_sources_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_customers_customer_sources_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer sources id verify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BankAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_sources_id_verify(
        &self,
        args: &PostCustomersCustomerSourcesIdVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BankAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_sources_id_verify_builder(
            &self.http_client,
            &args.customer,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_sources_id_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer subscriptions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_subscriptions(
        &self,
        args: &PostCustomersCustomerSubscriptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_subscriptions_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_subscriptions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer subscriptions subscription exposed id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_subscriptions_subscription_exposed_id(
        &self,
        args: &PostCustomersCustomerSubscriptionsSubscriptionExposedIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_subscriptions_subscription_exposed_id_builder(
            &self.http_client,
            &args.customer,
            &args.subscription_exposed_id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_subscriptions_subscription_exposed_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete customers customer subscriptions subscription exposed id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_customers_customer_subscriptions_subscription_exposed_id(
        &self,
        args: &DeleteCustomersCustomerSubscriptionsSubscriptionExposedIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_customers_customer_subscriptions_subscription_exposed_id_builder(
            &self.http_client,
            &args.customer,
            &args.subscription_exposed_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_customers_customer_subscriptions_subscription_exposed_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete customers customer subscriptions subscription exposed id discount.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedDiscount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_customers_customer_subscriptions_subscription_exposed_id_discount(
        &self,
        args: &DeleteCustomersCustomerSubscriptionsSubscriptionExposedIdDiscountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedDiscount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_customers_customer_subscriptions_subscription_exposed_id_discount_builder(
            &self.http_client,
            &args.customer,
            &args.subscription_exposed_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_customers_customer_subscriptions_subscription_exposed_id_discount_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post customers customer tax ids.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxId result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_tax_ids(
        &self,
        args: &PostCustomersCustomerTaxIdsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxId, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_customers_customer_tax_ids_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_customers_customer_tax_ids_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete customers customer tax ids id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedTaxId result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_customers_customer_tax_ids_id(
        &self,
        args: &DeleteCustomersCustomerTaxIdsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedTaxId, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_customers_customer_tax_ids_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_customers_customer_tax_ids_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post disputes dispute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dispute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_disputes_dispute(
        &self,
        args: &PostDisputesDisputeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_disputes_dispute_builder(
            &self.http_client,
            &args.dispute,
        )
        .map_err(ProviderError::Api)?;

        let task = post_disputes_dispute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post disputes dispute close.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dispute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_disputes_dispute_close(
        &self,
        args: &PostDisputesDisputeCloseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_disputes_dispute_close_builder(
            &self.http_client,
            &args.dispute,
        )
        .map_err(ProviderError::Api)?;

        let task = post_disputes_dispute_close_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post entitlements features.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntitlementsFeature result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_entitlements_features(
        &self,
        args: &PostEntitlementsFeaturesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntitlementsFeature, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_entitlements_features_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_entitlements_features_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post entitlements features id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntitlementsFeature result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_entitlements_features_id(
        &self,
        args: &PostEntitlementsFeaturesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntitlementsFeature, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_entitlements_features_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_entitlements_features_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post ephemeral keys.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EphemeralKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_ephemeral_keys(
        &self,
        args: &PostEphemeralKeysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EphemeralKey, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_ephemeral_keys_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_ephemeral_keys_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete ephemeral keys key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EphemeralKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_ephemeral_keys_key(
        &self,
        args: &DeleteEphemeralKeysKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EphemeralKey, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_ephemeral_keys_key_builder(
            &self.http_client,
            &args.key,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_ephemeral_keys_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post external accounts id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_external_accounts_id(
        &self,
        args: &PostExternalAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_external_accounts_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_external_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post file links.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FileLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_file_links(
        &self,
        args: &PostFileLinksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FileLink, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_file_links_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_file_links_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post file links link.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FileLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_file_links_link(
        &self,
        args: &PostFileLinksLinkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FileLink, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_file_links_link_builder(
            &self.http_client,
            &args.link,
        )
        .map_err(ProviderError::Api)?;

        let task = post_file_links_link_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post files.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the File result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_files(
        &self,
        args: &PostFilesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<File, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_files_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_files_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post financial connections accounts account disconnect.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinancialConnectionsAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_financial_connections_accounts_account_disconnect(
        &self,
        args: &PostFinancialConnectionsAccountsAccountDisconnectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_financial_connections_accounts_account_disconnect_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_financial_connections_accounts_account_disconnect_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post financial connections accounts account refresh.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinancialConnectionsAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_financial_connections_accounts_account_refresh(
        &self,
        args: &PostFinancialConnectionsAccountsAccountRefreshArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_financial_connections_accounts_account_refresh_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_financial_connections_accounts_account_refresh_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post financial connections accounts account subscribe.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinancialConnectionsAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_financial_connections_accounts_account_subscribe(
        &self,
        args: &PostFinancialConnectionsAccountsAccountSubscribeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_financial_connections_accounts_account_subscribe_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_financial_connections_accounts_account_subscribe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post financial connections accounts account unsubscribe.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinancialConnectionsAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_financial_connections_accounts_account_unsubscribe(
        &self,
        args: &PostFinancialConnectionsAccountsAccountUnsubscribeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_financial_connections_accounts_account_unsubscribe_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_financial_connections_accounts_account_unsubscribe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post financial connections sessions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinancialConnectionsSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_financial_connections_sessions(
        &self,
        args: &PostFinancialConnectionsSessionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_financial_connections_sessions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_financial_connections_sessions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post forwarding requests.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ForwardingRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_forwarding_requests(
        &self,
        args: &PostForwardingRequestsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ForwardingRequest, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_forwarding_requests_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_forwarding_requests_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post identity verification sessions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentityVerificationSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_identity_verification_sessions(
        &self,
        args: &PostIdentityVerificationSessionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentityVerificationSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_identity_verification_sessions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_identity_verification_sessions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post identity verification sessions session.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentityVerificationSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_identity_verification_sessions_session(
        &self,
        args: &PostIdentityVerificationSessionsSessionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentityVerificationSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_identity_verification_sessions_session_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = post_identity_verification_sessions_session_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post identity verification sessions session cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentityVerificationSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_identity_verification_sessions_session_cancel(
        &self,
        args: &PostIdentityVerificationSessionsSessionCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentityVerificationSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_identity_verification_sessions_session_cancel_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = post_identity_verification_sessions_session_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post identity verification sessions session redact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentityVerificationSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_identity_verification_sessions_session_redact(
        &self,
        args: &PostIdentityVerificationSessionsSessionRedactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentityVerificationSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_identity_verification_sessions_session_redact_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = post_identity_verification_sessions_session_redact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoice rendering templates template archive.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InvoiceRenderingTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoice_rendering_templates_template_archive(
        &self,
        args: &PostInvoiceRenderingTemplatesTemplateArchiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InvoiceRenderingTemplate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoice_rendering_templates_template_archive_builder(
            &self.http_client,
            &args.template,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoice_rendering_templates_template_archive_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoice rendering templates template unarchive.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InvoiceRenderingTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoice_rendering_templates_template_unarchive(
        &self,
        args: &PostInvoiceRenderingTemplatesTemplateUnarchiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InvoiceRenderingTemplate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoice_rendering_templates_template_unarchive_builder(
            &self.http_client,
            &args.template,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoice_rendering_templates_template_unarchive_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoiceitems.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoiceitem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoiceitems(
        &self,
        args: &PostInvoiceitemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoiceitem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoiceitems_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoiceitems_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoiceitems invoiceitem.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoiceitem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoiceitems_invoiceitem(
        &self,
        args: &PostInvoiceitemsInvoiceitemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoiceitem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoiceitems_invoiceitem_builder(
            &self.http_client,
            &args.invoiceitem,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoiceitems_invoiceitem_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete invoiceitems invoiceitem.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedInvoiceitem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_invoiceitems_invoiceitem(
        &self,
        args: &DeleteInvoiceitemsInvoiceitemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedInvoiceitem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_invoiceitems_invoiceitem_builder(
            &self.http_client,
            &args.invoiceitem,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_invoiceitems_invoiceitem_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices(
        &self,
        args: &PostInvoicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices create preview.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_create_preview(
        &self,
        args: &PostInvoicesCreatePreviewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_create_preview_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_create_preview_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice(
        &self,
        args: &PostInvoicesInvoiceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete invoices invoice.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedInvoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_invoices_invoice(
        &self,
        args: &DeleteInvoicesInvoiceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedInvoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_invoices_invoice_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_invoices_invoice_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice add lines.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_add_lines(
        &self,
        args: &PostInvoicesInvoiceAddLinesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_add_lines_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_add_lines_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice attach payment.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_attach_payment(
        &self,
        args: &PostInvoicesInvoiceAttachPaymentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_attach_payment_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_attach_payment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice finalize.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_finalize(
        &self,
        args: &PostInvoicesInvoiceFinalizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_finalize_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_finalize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice lines line item id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LineItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_lines_line_item_id(
        &self,
        args: &PostInvoicesInvoiceLinesLineItemIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LineItem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_lines_line_item_id_builder(
            &self.http_client,
            &args.invoice,
            &args.line_item_id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_lines_line_item_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice mark uncollectible.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_mark_uncollectible(
        &self,
        args: &PostInvoicesInvoiceMarkUncollectibleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_mark_uncollectible_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_mark_uncollectible_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice pay.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_pay(
        &self,
        args: &PostInvoicesInvoicePayArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_pay_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_pay_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice remove lines.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_remove_lines(
        &self,
        args: &PostInvoicesInvoiceRemoveLinesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_remove_lines_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_remove_lines_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice send.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_send(
        &self,
        args: &PostInvoicesInvoiceSendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_send_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_send_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice update lines.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_update_lines(
        &self,
        args: &PostInvoicesInvoiceUpdateLinesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_update_lines_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_update_lines_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoices invoice void.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Invoice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_invoices_invoice_void(
        &self,
        args: &PostInvoicesInvoiceVoidArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_invoices_invoice_void_builder(
            &self.http_client,
            &args.invoice,
        )
        .map_err(ProviderError::Api)?;

        let task = post_invoices_invoice_void_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing authorizations authorization.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_authorizations_authorization(
        &self,
        args: &PostIssuingAuthorizationsAuthorizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_authorizations_authorization_builder(
            &self.http_client,
            &args.authorization,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_authorizations_authorization_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing authorizations authorization approve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_authorizations_authorization_approve(
        &self,
        args: &PostIssuingAuthorizationsAuthorizationApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_authorizations_authorization_approve_builder(
            &self.http_client,
            &args.authorization,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_authorizations_authorization_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing authorizations authorization decline.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_authorizations_authorization_decline(
        &self,
        args: &PostIssuingAuthorizationsAuthorizationDeclineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_authorizations_authorization_decline_builder(
            &self.http_client,
            &args.authorization,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_authorizations_authorization_decline_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing cardholders.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingCardholder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_cardholders(
        &self,
        args: &PostIssuingCardholdersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCardholder, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_cardholders_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_cardholders_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing cardholders cardholder.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingCardholder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_cardholders_cardholder(
        &self,
        args: &PostIssuingCardholdersCardholderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCardholder, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_cardholders_cardholder_builder(
            &self.http_client,
            &args.cardholder,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_cardholders_cardholder_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing cards.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingCard result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_cards(
        &self,
        args: &PostIssuingCardsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCard, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_cards_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_cards_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing cards card.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingCard result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_cards_card(
        &self,
        args: &PostIssuingCardsCardArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCard, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_cards_card_builder(
            &self.http_client,
            &args.card,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_cards_card_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing disputes.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingDispute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_disputes(
        &self,
        args: &PostIssuingDisputesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingDispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_disputes_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_disputes_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing disputes dispute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingDispute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_disputes_dispute(
        &self,
        args: &PostIssuingDisputesDisputeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingDispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_disputes_dispute_builder(
            &self.http_client,
            &args.dispute,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_disputes_dispute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing disputes dispute submit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingDispute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_disputes_dispute_submit(
        &self,
        args: &PostIssuingDisputesDisputeSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingDispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_disputes_dispute_submit_builder(
            &self.http_client,
            &args.dispute,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_disputes_dispute_submit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing personalization designs.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingPersonalizationDesign result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_personalization_designs(
        &self,
        args: &PostIssuingPersonalizationDesignsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingPersonalizationDesign, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_personalization_designs_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_personalization_designs_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing personalization designs personalization design.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingPersonalizationDesign result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_personalization_designs_personalization_design(
        &self,
        args: &PostIssuingPersonalizationDesignsPersonalizationDesignArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingPersonalizationDesign, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_personalization_designs_personalization_design_builder(
            &self.http_client,
            &args.personalization_design,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_personalization_designs_personalization_design_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing settlements settlement.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingSettlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_settlements_settlement(
        &self,
        args: &PostIssuingSettlementsSettlementArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingSettlement, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_settlements_settlement_builder(
            &self.http_client,
            &args.settlement,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_settlements_settlement_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing tokens token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_tokens_token(
        &self,
        args: &PostIssuingTokensTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingToken, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_tokens_token_builder(
            &self.http_client,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_tokens_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post issuing transactions transaction.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_issuing_transactions_transaction(
        &self,
        args: &PostIssuingTransactionsTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_issuing_transactions_transaction_builder(
            &self.http_client,
            &args.transaction,
        )
        .map_err(ProviderError::Api)?;

        let task = post_issuing_transactions_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post link account sessions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinancialConnectionsSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_link_account_sessions(
        &self,
        args: &PostLinkAccountSessionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_link_account_sessions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_link_account_sessions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post linked accounts account disconnect.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinancialConnectionsAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_linked_accounts_account_disconnect(
        &self,
        args: &PostLinkedAccountsAccountDisconnectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_linked_accounts_account_disconnect_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_linked_accounts_account_disconnect_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post linked accounts account refresh.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinancialConnectionsAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_linked_accounts_account_refresh(
        &self,
        args: &PostLinkedAccountsAccountRefreshArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_linked_accounts_account_refresh_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_linked_accounts_account_refresh_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment intents.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_intents(
        &self,
        args: &PostPaymentIntentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_intents_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_intents_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment intents intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_intents_intent(
        &self,
        args: &PostPaymentIntentsIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_intents_intent_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_intents_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment intents intent apply customer balance.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_intents_intent_apply_customer_balance(
        &self,
        args: &PostPaymentIntentsIntentApplyCustomerBalanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_intents_intent_apply_customer_balance_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_intents_intent_apply_customer_balance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment intents intent cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_intents_intent_cancel(
        &self,
        args: &PostPaymentIntentsIntentCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_intents_intent_cancel_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_intents_intent_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment intents intent capture.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_intents_intent_capture(
        &self,
        args: &PostPaymentIntentsIntentCaptureArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_intents_intent_capture_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_intents_intent_capture_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment intents intent confirm.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_intents_intent_confirm(
        &self,
        args: &PostPaymentIntentsIntentConfirmArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_intents_intent_confirm_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_intents_intent_confirm_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment intents intent increment authorization.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_intents_intent_increment_authorization(
        &self,
        args: &PostPaymentIntentsIntentIncrementAuthorizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_intents_intent_increment_authorization_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_intents_intent_increment_authorization_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment intents intent verify microdeposits.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_intents_intent_verify_microdeposits(
        &self,
        args: &PostPaymentIntentsIntentVerifyMicrodepositsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_intents_intent_verify_microdeposits_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_intents_intent_verify_microdeposits_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment links.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_links(
        &self,
        args: &PostPaymentLinksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentLink, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_links_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_links_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment links payment link.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_links_payment_link(
        &self,
        args: &PostPaymentLinksPaymentLinkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentLink, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_links_payment_link_builder(
            &self.http_client,
            &args.payment_link,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_links_payment_link_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment method configurations.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentMethodConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_method_configurations(
        &self,
        args: &PostPaymentMethodConfigurationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethodConfiguration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_method_configurations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_method_configurations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment method configurations configuration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentMethodConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_method_configurations_configuration(
        &self,
        args: &PostPaymentMethodConfigurationsConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethodConfiguration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_method_configurations_configuration_builder(
            &self.http_client,
            &args.configuration,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_method_configurations_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment method domains.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentMethodDomain result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_method_domains(
        &self,
        args: &PostPaymentMethodDomainsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethodDomain, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_method_domains_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_method_domains_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment method domains payment method domain.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentMethodDomain result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_method_domains_payment_method_domain(
        &self,
        args: &PostPaymentMethodDomainsPaymentMethodDomainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethodDomain, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_method_domains_payment_method_domain_builder(
            &self.http_client,
            &args.payment_method_domain,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_method_domains_payment_method_domain_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment method domains payment method domain validate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentMethodDomain result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_method_domains_payment_method_domain_validate(
        &self,
        args: &PostPaymentMethodDomainsPaymentMethodDomainValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethodDomain, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_method_domains_payment_method_domain_validate_builder(
            &self.http_client,
            &args.payment_method_domain,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_method_domains_payment_method_domain_validate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment methods.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentMethod result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_methods(
        &self,
        args: &PostPaymentMethodsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethod, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_methods_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_methods_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment methods payment method.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentMethod result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_methods_payment_method(
        &self,
        args: &PostPaymentMethodsPaymentMethodArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethod, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_methods_payment_method_builder(
            &self.http_client,
            &args.payment_method,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_methods_payment_method_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment methods payment method attach.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentMethod result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_methods_payment_method_attach(
        &self,
        args: &PostPaymentMethodsPaymentMethodAttachArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethod, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_methods_payment_method_attach_builder(
            &self.http_client,
            &args.payment_method,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_methods_payment_method_attach_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment methods payment method detach.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentMethod result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_methods_payment_method_detach(
        &self,
        args: &PostPaymentMethodsPaymentMethodDetachArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethod, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_methods_payment_method_detach_builder(
            &self.http_client,
            &args.payment_method,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_methods_payment_method_detach_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment records report payment.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_records_report_payment(
        &self,
        args: &PostPaymentRecordsReportPaymentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentRecord, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_records_report_payment_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_records_report_payment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment records id report payment attempt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_records_id_report_payment_attempt(
        &self,
        args: &PostPaymentRecordsIdReportPaymentAttemptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentRecord, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_records_id_report_payment_attempt_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_records_id_report_payment_attempt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment records id report payment attempt canceled.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_records_id_report_payment_attempt_canceled(
        &self,
        args: &PostPaymentRecordsIdReportPaymentAttemptCanceledArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentRecord, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_records_id_report_payment_attempt_canceled_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_records_id_report_payment_attempt_canceled_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment records id report payment attempt failed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_records_id_report_payment_attempt_failed(
        &self,
        args: &PostPaymentRecordsIdReportPaymentAttemptFailedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentRecord, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_records_id_report_payment_attempt_failed_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_records_id_report_payment_attempt_failed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment records id report payment attempt guaranteed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_records_id_report_payment_attempt_guaranteed(
        &self,
        args: &PostPaymentRecordsIdReportPaymentAttemptGuaranteedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentRecord, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_records_id_report_payment_attempt_guaranteed_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_records_id_report_payment_attempt_guaranteed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment records id report payment attempt informational.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_records_id_report_payment_attempt_informational(
        &self,
        args: &PostPaymentRecordsIdReportPaymentAttemptInformationalArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentRecord, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_records_id_report_payment_attempt_informational_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_records_id_report_payment_attempt_informational_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payment records id report refund.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payment_records_id_report_refund(
        &self,
        args: &PostPaymentRecordsIdReportRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentRecord, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payment_records_id_report_refund_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payment_records_id_report_refund_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payouts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Payout result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payouts(
        &self,
        args: &PostPayoutsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Payout, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payouts_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payouts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payouts payout.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Payout result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payouts_payout(
        &self,
        args: &PostPayoutsPayoutArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Payout, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payouts_payout_builder(
            &self.http_client,
            &args.payout,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payouts_payout_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payouts payout cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Payout result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payouts_payout_cancel(
        &self,
        args: &PostPayoutsPayoutCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Payout, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payouts_payout_cancel_builder(
            &self.http_client,
            &args.payout,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payouts_payout_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post payouts payout reverse.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Payout result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_payouts_payout_reverse(
        &self,
        args: &PostPayoutsPayoutReverseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Payout, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_payouts_payout_reverse_builder(
            &self.http_client,
            &args.payout,
        )
        .map_err(ProviderError::Api)?;

        let task = post_payouts_payout_reverse_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post plans.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Plan result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_plans(
        &self,
        args: &PostPlansArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Plan, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_plans_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_plans_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post plans plan.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Plan result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_plans_plan(
        &self,
        args: &PostPlansPlanArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Plan, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_plans_plan_builder(
            &self.http_client,
            &args.plan,
        )
        .map_err(ProviderError::Api)?;

        let task = post_plans_plan_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete plans plan.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedPlan result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_plans_plan(
        &self,
        args: &DeletePlansPlanArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedPlan, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_plans_plan_builder(
            &self.http_client,
            &args.plan,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_plans_plan_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post prices.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Price result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_prices(
        &self,
        args: &PostPricesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Price, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_prices_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_prices_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post prices price.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Price result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_prices_price(
        &self,
        args: &PostPricesPriceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Price, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_prices_price_builder(
            &self.http_client,
            &args.price,
        )
        .map_err(ProviderError::Api)?;

        let task = post_prices_price_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post products.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Product result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_products(
        &self,
        args: &PostProductsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_products_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_products_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post products id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Product result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_products_id(
        &self,
        args: &PostProductsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_products_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_products_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete products id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedProduct result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_products_id(
        &self,
        args: &DeleteProductsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedProduct, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_products_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_products_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post products product features.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductFeature result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_products_product_features(
        &self,
        args: &PostProductsProductFeaturesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductFeature, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_products_product_features_builder(
            &self.http_client,
            &args.product,
        )
        .map_err(ProviderError::Api)?;

        let task = post_products_product_features_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete products product features id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedProductFeature result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_products_product_features_id(
        &self,
        args: &DeleteProductsProductFeaturesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedProductFeature, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_products_product_features_id_builder(
            &self.http_client,
            &args.id,
            &args.product,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_products_product_features_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post promotion codes.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PromotionCode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_promotion_codes(
        &self,
        args: &PostPromotionCodesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PromotionCode, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_promotion_codes_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_promotion_codes_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post promotion codes promotion code.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PromotionCode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_promotion_codes_promotion_code(
        &self,
        args: &PostPromotionCodesPromotionCodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PromotionCode, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_promotion_codes_promotion_code_builder(
            &self.http_client,
            &args.promotion_code,
        )
        .map_err(ProviderError::Api)?;

        let task = post_promotion_codes_promotion_code_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post quotes.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Quote result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_quotes(
        &self,
        args: &PostQuotesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Quote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_quotes_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_quotes_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post quotes quote.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Quote result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_quotes_quote(
        &self,
        args: &PostQuotesQuoteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Quote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_quotes_quote_builder(
            &self.http_client,
            &args.quote,
        )
        .map_err(ProviderError::Api)?;

        let task = post_quotes_quote_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post quotes quote accept.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Quote result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_quotes_quote_accept(
        &self,
        args: &PostQuotesQuoteAcceptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Quote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_quotes_quote_accept_builder(
            &self.http_client,
            &args.quote,
        )
        .map_err(ProviderError::Api)?;

        let task = post_quotes_quote_accept_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post quotes quote cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Quote result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_quotes_quote_cancel(
        &self,
        args: &PostQuotesQuoteCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Quote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_quotes_quote_cancel_builder(
            &self.http_client,
            &args.quote,
        )
        .map_err(ProviderError::Api)?;

        let task = post_quotes_quote_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post quotes quote finalize.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Quote result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_quotes_quote_finalize(
        &self,
        args: &PostQuotesQuoteFinalizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Quote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_quotes_quote_finalize_builder(
            &self.http_client,
            &args.quote,
        )
        .map_err(ProviderError::Api)?;

        let task = post_quotes_quote_finalize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post radar payment evaluations.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RadarPaymentEvaluation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_radar_payment_evaluations(
        &self,
        args: &PostRadarPaymentEvaluationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RadarPaymentEvaluation, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_radar_payment_evaluations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_radar_payment_evaluations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post radar value list items.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RadarValueListItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_radar_value_list_items(
        &self,
        args: &PostRadarValueListItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RadarValueListItem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_radar_value_list_items_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_radar_value_list_items_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete radar value list items item.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedRadarValueListItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_radar_value_list_items_item(
        &self,
        args: &DeleteRadarValueListItemsItemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedRadarValueListItem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_radar_value_list_items_item_builder(
            &self.http_client,
            &args.item,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_radar_value_list_items_item_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post radar value lists.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RadarValueList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_radar_value_lists(
        &self,
        args: &PostRadarValueListsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RadarValueList, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_radar_value_lists_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_radar_value_lists_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post radar value lists value list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RadarValueList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_radar_value_lists_value_list(
        &self,
        args: &PostRadarValueListsValueListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RadarValueList, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_radar_value_lists_value_list_builder(
            &self.http_client,
            &args.value_list,
        )
        .map_err(ProviderError::Api)?;

        let task = post_radar_value_lists_value_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete radar value lists value list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedRadarValueList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_radar_value_lists_value_list(
        &self,
        args: &DeleteRadarValueListsValueListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedRadarValueList, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_radar_value_lists_value_list_builder(
            &self.http_client,
            &args.value_list,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_radar_value_lists_value_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post refunds.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Refund result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_refunds(
        &self,
        args: &PostRefundsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Refund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_refunds_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_refunds_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post refunds refund.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Refund result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_refunds_refund(
        &self,
        args: &PostRefundsRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Refund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_refunds_refund_builder(
            &self.http_client,
            &args.refund,
        )
        .map_err(ProviderError::Api)?;

        let task = post_refunds_refund_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post refunds refund cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Refund result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_refunds_refund_cancel(
        &self,
        args: &PostRefundsRefundCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Refund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_refunds_refund_cancel_builder(
            &self.http_client,
            &args.refund,
        )
        .map_err(ProviderError::Api)?;

        let task = post_refunds_refund_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post reporting report runs.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportingReportRun result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_reporting_report_runs(
        &self,
        args: &PostReportingReportRunsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportingReportRun, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_reporting_report_runs_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_reporting_report_runs_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post reviews review approve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Review result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_reviews_review_approve(
        &self,
        args: &PostReviewsReviewApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Review, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_reviews_review_approve_builder(
            &self.http_client,
            &args.review,
        )
        .map_err(ProviderError::Api)?;

        let task = post_reviews_review_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post setup intents.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetupIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_setup_intents(
        &self,
        args: &PostSetupIntentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetupIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_setup_intents_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_setup_intents_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post setup intents intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetupIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_setup_intents_intent(
        &self,
        args: &PostSetupIntentsIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetupIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_setup_intents_intent_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_setup_intents_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post setup intents intent cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetupIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_setup_intents_intent_cancel(
        &self,
        args: &PostSetupIntentsIntentCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetupIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_setup_intents_intent_cancel_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_setup_intents_intent_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post setup intents intent confirm.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetupIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_setup_intents_intent_confirm(
        &self,
        args: &PostSetupIntentsIntentConfirmArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetupIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_setup_intents_intent_confirm_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_setup_intents_intent_confirm_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post setup intents intent verify microdeposits.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetupIntent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_setup_intents_intent_verify_microdeposits(
        &self,
        args: &PostSetupIntentsIntentVerifyMicrodepositsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetupIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_setup_intents_intent_verify_microdeposits_builder(
            &self.http_client,
            &args.intent,
        )
        .map_err(ProviderError::Api)?;

        let task = post_setup_intents_intent_verify_microdeposits_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post shipping rates.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShippingRate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_shipping_rates(
        &self,
        args: &PostShippingRatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingRate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_shipping_rates_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_shipping_rates_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post shipping rates shipping rate token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShippingRate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_shipping_rates_shipping_rate_token(
        &self,
        args: &PostShippingRatesShippingRateTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingRate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_shipping_rates_shipping_rate_token_builder(
            &self.http_client,
            &args.shipping_rate_token,
        )
        .map_err(ProviderError::Api)?;

        let task = post_shipping_rates_shipping_rate_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post sigma saved queries id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SigmaSigmaApiQuery result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_sigma_saved_queries_id(
        &self,
        args: &PostSigmaSavedQueriesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SigmaSigmaApiQuery, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_sigma_saved_queries_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_sigma_saved_queries_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post sources.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Source result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_sources(
        &self,
        args: &PostSourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Source, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_sources_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_sources_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post sources source.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Source result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_sources_source(
        &self,
        args: &PostSourcesSourceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Source, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_sources_source_builder(
            &self.http_client,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = post_sources_source_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post sources source verify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Source result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_sources_source_verify(
        &self,
        args: &PostSourcesSourceVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Source, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_sources_source_verify_builder(
            &self.http_client,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = post_sources_source_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscription items.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscription_items(
        &self,
        args: &PostSubscriptionItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionItem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscription_items_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscription_items_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscription items item.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscription_items_item(
        &self,
        args: &PostSubscriptionItemsItemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionItem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscription_items_item_builder(
            &self.http_client,
            &args.item,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscription_items_item_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete subscription items item.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedSubscriptionItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_subscription_items_item(
        &self,
        args: &DeleteSubscriptionItemsItemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedSubscriptionItem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_subscription_items_item_builder(
            &self.http_client,
            &args.item,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_subscription_items_item_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscription schedules.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscription_schedules(
        &self,
        args: &PostSubscriptionSchedulesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionSchedule, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscription_schedules_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscription_schedules_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscription schedules schedule.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscription_schedules_schedule(
        &self,
        args: &PostSubscriptionSchedulesScheduleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionSchedule, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscription_schedules_schedule_builder(
            &self.http_client,
            &args.schedule,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscription_schedules_schedule_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscription schedules schedule cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscription_schedules_schedule_cancel(
        &self,
        args: &PostSubscriptionSchedulesScheduleCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionSchedule, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscription_schedules_schedule_cancel_builder(
            &self.http_client,
            &args.schedule,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscription_schedules_schedule_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscription schedules schedule release.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscription_schedules_schedule_release(
        &self,
        args: &PostSubscriptionSchedulesScheduleReleaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionSchedule, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscription_schedules_schedule_release_builder(
            &self.http_client,
            &args.schedule,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscription_schedules_schedule_release_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscriptions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscriptions(
        &self,
        args: &PostSubscriptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscriptions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscriptions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscriptions subscription exposed id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscriptions_subscription_exposed_id(
        &self,
        args: &PostSubscriptionsSubscriptionExposedIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscriptions_subscription_exposed_id_builder(
            &self.http_client,
            &args.subscription_exposed_id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscriptions_subscription_exposed_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete subscriptions subscription exposed id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_subscriptions_subscription_exposed_id(
        &self,
        args: &DeleteSubscriptionsSubscriptionExposedIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_subscriptions_subscription_exposed_id_builder(
            &self.http_client,
            &args.subscription_exposed_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_subscriptions_subscription_exposed_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete subscriptions subscription exposed id discount.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedDiscount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_subscriptions_subscription_exposed_id_discount(
        &self,
        args: &DeleteSubscriptionsSubscriptionExposedIdDiscountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedDiscount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_subscriptions_subscription_exposed_id_discount_builder(
            &self.http_client,
            &args.subscription_exposed_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_subscriptions_subscription_exposed_id_discount_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscriptions subscription migrate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscriptions_subscription_migrate(
        &self,
        args: &PostSubscriptionsSubscriptionMigrateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscriptions_subscription_migrate_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscriptions_subscription_migrate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post subscriptions subscription resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_subscriptions_subscription_resume(
        &self,
        args: &PostSubscriptionsSubscriptionResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_subscriptions_subscription_resume_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = post_subscriptions_subscription_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax calculations.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxCalculation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tax_calculations(
        &self,
        args: &PostTaxCalculationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxCalculation, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tax_calculations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tax_calculations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax registrations.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxRegistration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tax_registrations(
        &self,
        args: &PostTaxRegistrationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxRegistration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tax_registrations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tax_registrations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax registrations id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxRegistration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tax_registrations_id(
        &self,
        args: &PostTaxRegistrationsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxRegistration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tax_registrations_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tax_registrations_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tax_settings(
        &self,
        args: &PostTaxSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxSettings, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tax_settings_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tax_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax transactions create from calculation.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tax_transactions_create_from_calculation(
        &self,
        args: &PostTaxTransactionsCreateFromCalculationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tax_transactions_create_from_calculation_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tax_transactions_create_from_calculation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax transactions create reversal.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tax_transactions_create_reversal(
        &self,
        args: &PostTaxTransactionsCreateReversalArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tax_transactions_create_reversal_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tax_transactions_create_reversal_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax ids.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxId result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tax_ids(
        &self,
        args: &PostTaxIdsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxId, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tax_ids_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tax_ids_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete tax ids id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedTaxId result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_tax_ids_id(
        &self,
        args: &DeleteTaxIdsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedTaxId, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_tax_ids_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_tax_ids_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax rates.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxRate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tax_rates(
        &self,
        args: &PostTaxRatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxRate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tax_rates_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tax_rates_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax rates tax rate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxRate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tax_rates_tax_rate(
        &self,
        args: &PostTaxRatesTaxRateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxRate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tax_rates_tax_rate_builder(
            &self.http_client,
            &args.tax_rate,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tax_rates_tax_rate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal configurations.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_configurations(
        &self,
        args: &PostTerminalConfigurationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalConfiguration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_configurations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_configurations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal configurations configuration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_configurations_configuration(
        &self,
        args: &PostTerminalConfigurationsConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_configurations_configuration_builder(
            &self.http_client,
            &args.configuration,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_configurations_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete terminal configurations configuration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedTerminalConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_terminal_configurations_configuration(
        &self,
        args: &DeleteTerminalConfigurationsConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedTerminalConfiguration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_terminal_configurations_configuration_builder(
            &self.http_client,
            &args.configuration,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_terminal_configurations_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal connection tokens.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalConnectionToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_connection_tokens(
        &self,
        args: &PostTerminalConnectionTokensArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalConnectionToken, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_connection_tokens_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_connection_tokens_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal locations.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalLocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_locations(
        &self,
        args: &PostTerminalLocationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalLocation, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_locations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_locations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal locations location.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_locations_location(
        &self,
        args: &PostTerminalLocationsLocationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_locations_location_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_locations_location_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete terminal locations location.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedTerminalLocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_terminal_locations_location(
        &self,
        args: &DeleteTerminalLocationsLocationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedTerminalLocation, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_terminal_locations_location_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_terminal_locations_location_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal onboarding links.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalOnboardingLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_onboarding_links(
        &self,
        args: &PostTerminalOnboardingLinksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalOnboardingLink, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_onboarding_links_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_onboarding_links_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers(
        &self,
        args: &PostTerminalReadersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers reader.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers_reader(
        &self,
        args: &PostTerminalReadersReaderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_reader_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_reader_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete terminal readers reader.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedTerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_terminal_readers_reader(
        &self,
        args: &DeleteTerminalReadersReaderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedTerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_terminal_readers_reader_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_terminal_readers_reader_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers reader cancel action.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers_reader_cancel_action(
        &self,
        args: &PostTerminalReadersReaderCancelActionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_reader_cancel_action_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_reader_cancel_action_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers reader collect inputs.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers_reader_collect_inputs(
        &self,
        args: &PostTerminalReadersReaderCollectInputsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_reader_collect_inputs_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_reader_collect_inputs_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers reader collect payment method.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers_reader_collect_payment_method(
        &self,
        args: &PostTerminalReadersReaderCollectPaymentMethodArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_reader_collect_payment_method_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_reader_collect_payment_method_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers reader confirm payment intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers_reader_confirm_payment_intent(
        &self,
        args: &PostTerminalReadersReaderConfirmPaymentIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_reader_confirm_payment_intent_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_reader_confirm_payment_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers reader process payment intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers_reader_process_payment_intent(
        &self,
        args: &PostTerminalReadersReaderProcessPaymentIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_reader_process_payment_intent_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_reader_process_payment_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers reader process setup intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers_reader_process_setup_intent(
        &self,
        args: &PostTerminalReadersReaderProcessSetupIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_reader_process_setup_intent_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_reader_process_setup_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers reader refund payment.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers_reader_refund_payment(
        &self,
        args: &PostTerminalReadersReaderRefundPaymentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_reader_refund_payment_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_reader_refund_payment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal readers reader set reader display.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_readers_reader_set_reader_display(
        &self,
        args: &PostTerminalReadersReaderSetReaderDisplayArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_readers_reader_set_reader_display_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_readers_reader_set_reader_display_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post terminal refunds.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalRefund result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_terminal_refunds(
        &self,
        args: &PostTerminalRefundsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalRefund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_terminal_refunds_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_terminal_refunds_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers confirmation tokens.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConfirmationToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_confirmation_tokens(
        &self,
        args: &PostTestHelpersConfirmationTokensArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConfirmationToken, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_confirmation_tokens_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_confirmation_tokens_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers customers customer fund cash balance.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerCashBalanceTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_customers_customer_fund_cash_balance(
        &self,
        args: &PostTestHelpersCustomersCustomerFundCashBalanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerCashBalanceTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_customers_customer_fund_cash_balance_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_customers_customer_fund_cash_balance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_authorizations(
        &self,
        args: &PostTestHelpersIssuingAuthorizationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_authorizations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_authorizations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization capture.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_authorizations_authorization_capture(
        &self,
        args: &PostTestHelpersIssuingAuthorizationsAuthorizationCaptureArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_authorizations_authorization_capture_builder(
            &self.http_client,
            &args.authorization,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_authorizations_authorization_capture_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization expire.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_authorizations_authorization_expire(
        &self,
        args: &PostTestHelpersIssuingAuthorizationsAuthorizationExpireArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_authorizations_authorization_expire_builder(
            &self.http_client,
            &args.authorization,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_authorizations_authorization_expire_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization finalize amount.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_authorizations_authorization_finalize_amount(
        &self,
        args: &PostTestHelpersIssuingAuthorizationsAuthorizationFinalizeAmountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_authorizations_authorization_finalize_amount_builder(
            &self.http_client,
            &args.authorization,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_authorizations_authorization_finalize_amount_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization fraud challenges respond.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_authorizations_authorization_fraud_challenges_respond(
        &self,
        args: &PostTestHelpersIssuingAuthorizationsAuthorizationFraudChallengesRespondArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_authorizations_authorization_fraud_challenges_respond_builder(
            &self.http_client,
            &args.authorization,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_authorizations_authorization_fraud_challenges_respond_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization increment.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_authorizations_authorization_increment(
        &self,
        args: &PostTestHelpersIssuingAuthorizationsAuthorizationIncrementArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_authorizations_authorization_increment_builder(
            &self.http_client,
            &args.authorization,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_authorizations_authorization_increment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization reverse.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingAuthorization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_authorizations_authorization_reverse(
        &self,
        args: &PostTestHelpersIssuingAuthorizationsAuthorizationReverseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_authorizations_authorization_reverse_builder(
            &self.http_client,
            &args.authorization,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_authorizations_authorization_reverse_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping deliver.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingCard result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_cards_card_shipping_deliver(
        &self,
        args: &PostTestHelpersIssuingCardsCardShippingDeliverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCard, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_cards_card_shipping_deliver_builder(
            &self.http_client,
            &args.card,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_cards_card_shipping_deliver_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping fail.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingCard result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_cards_card_shipping_fail(
        &self,
        args: &PostTestHelpersIssuingCardsCardShippingFailArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCard, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_cards_card_shipping_fail_builder(
            &self.http_client,
            &args.card,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_cards_card_shipping_fail_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping return.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingCard result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_cards_card_shipping_return(
        &self,
        args: &PostTestHelpersIssuingCardsCardShippingReturnArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCard, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_cards_card_shipping_return_builder(
            &self.http_client,
            &args.card,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_cards_card_shipping_return_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping ship.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingCard result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_cards_card_shipping_ship(
        &self,
        args: &PostTestHelpersIssuingCardsCardShippingShipArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCard, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_cards_card_shipping_ship_builder(
            &self.http_client,
            &args.card,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_cards_card_shipping_ship_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping submit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingCard result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_cards_card_shipping_submit(
        &self,
        args: &PostTestHelpersIssuingCardsCardShippingSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCard, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_cards_card_shipping_submit_builder(
            &self.http_client,
            &args.card,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_cards_card_shipping_submit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing personalization designs personalization design activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingPersonalizationDesign result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_personalization_designs_personalization_design_activate(
        &self,
        args: &PostTestHelpersIssuingPersonalizationDesignsPersonalizationDesignActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingPersonalizationDesign, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_personalization_designs_personalization_design_activate_builder(
            &self.http_client,
            &args.personalization_design,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_personalization_designs_personalization_design_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing personalization designs personalization design deactivate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingPersonalizationDesign result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_personalization_designs_personalization_design_deactivate(
        &self,
        args: &PostTestHelpersIssuingPersonalizationDesignsPersonalizationDesignDeactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingPersonalizationDesign, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_personalization_designs_personalization_design_deactivate_builder(
            &self.http_client,
            &args.personalization_design,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_personalization_designs_personalization_design_deactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing personalization designs personalization design reject.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingPersonalizationDesign result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_personalization_designs_personalization_design_reject(
        &self,
        args: &PostTestHelpersIssuingPersonalizationDesignsPersonalizationDesignRejectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingPersonalizationDesign, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_personalization_designs_personalization_design_reject_builder(
            &self.http_client,
            &args.personalization_design,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_personalization_designs_personalization_design_reject_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing settlements.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingSettlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_settlements(
        &self,
        args: &PostTestHelpersIssuingSettlementsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingSettlement, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_settlements_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_settlements_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing settlements settlement complete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingSettlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_settlements_settlement_complete(
        &self,
        args: &PostTestHelpersIssuingSettlementsSettlementCompleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingSettlement, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_settlements_settlement_complete_builder(
            &self.http_client,
            &args.settlement,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_settlements_settlement_complete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing transactions create force capture.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_transactions_create_force_capture(
        &self,
        args: &PostTestHelpersIssuingTransactionsCreateForceCaptureArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_transactions_create_force_capture_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_transactions_create_force_capture_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing transactions create unlinked refund.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_transactions_create_unlinked_refund(
        &self,
        args: &PostTestHelpersIssuingTransactionsCreateUnlinkedRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_transactions_create_unlinked_refund_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_transactions_create_unlinked_refund_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing transactions transaction refund.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_issuing_transactions_transaction_refund(
        &self,
        args: &PostTestHelpersIssuingTransactionsTransactionRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_issuing_transactions_transaction_refund_builder(
            &self.http_client,
            &args.transaction,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_issuing_transactions_transaction_refund_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers refunds refund expire.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Refund result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_refunds_refund_expire(
        &self,
        args: &PostTestHelpersRefundsRefundExpireArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Refund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_refunds_refund_expire_builder(
            &self.http_client,
            &args.refund,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_refunds_refund_expire_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers terminal readers reader present payment method.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_terminal_readers_reader_present_payment_method(
        &self,
        args: &PostTestHelpersTerminalReadersReaderPresentPaymentMethodArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_terminal_readers_reader_present_payment_method_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_terminal_readers_reader_present_payment_method_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers terminal readers reader succeed input collection.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_terminal_readers_reader_succeed_input_collection(
        &self,
        args: &PostTestHelpersTerminalReadersReaderSucceedInputCollectionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_terminal_readers_reader_succeed_input_collection_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_terminal_readers_reader_succeed_input_collection_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers terminal readers reader timeout input collection.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminalReader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_terminal_readers_reader_timeout_input_collection(
        &self,
        args: &PostTestHelpersTerminalReadersReaderTimeoutInputCollectionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminalReader, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_terminal_readers_reader_timeout_input_collection_builder(
            &self.http_client,
            &args.reader,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_terminal_readers_reader_timeout_input_collection_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers test clocks.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestHelpersTestClock result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_test_clocks(
        &self,
        args: &PostTestHelpersTestClocksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestHelpersTestClock, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_test_clocks_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_test_clocks_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete test helpers test clocks test clock.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedTestHelpersTestClock result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_test_helpers_test_clocks_test_clock(
        &self,
        args: &DeleteTestHelpersTestClocksTestClockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedTestHelpersTestClock, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_test_helpers_test_clocks_test_clock_builder(
            &self.http_client,
            &args.test_clock,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_test_helpers_test_clocks_test_clock_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers test clocks test clock advance.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestHelpersTestClock result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_test_clocks_test_clock_advance(
        &self,
        args: &PostTestHelpersTestClocksTestClockAdvanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestHelpersTestClock, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_test_clocks_test_clock_advance_builder(
            &self.http_client,
            &args.test_clock,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_test_clocks_test_clock_advance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury inbound transfers id fail.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryInboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_inbound_transfers_id_fail(
        &self,
        args: &PostTestHelpersTreasuryInboundTransfersIdFailArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryInboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_inbound_transfers_id_fail_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_inbound_transfers_id_fail_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury inbound transfers id return.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryInboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_inbound_transfers_id_return(
        &self,
        args: &PostTestHelpersTreasuryInboundTransfersIdReturnArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryInboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_inbound_transfers_id_return_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_inbound_transfers_id_return_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury inbound transfers id succeed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryInboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_inbound_transfers_id_succeed(
        &self,
        args: &PostTestHelpersTreasuryInboundTransfersIdSucceedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryInboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_inbound_transfers_id_succeed_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_inbound_transfers_id_succeed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound payments id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundPayment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_outbound_payments_id(
        &self,
        args: &PostTestHelpersTreasuryOutboundPaymentsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundPayment, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_outbound_payments_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_outbound_payments_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound payments id fail.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundPayment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_outbound_payments_id_fail(
        &self,
        args: &PostTestHelpersTreasuryOutboundPaymentsIdFailArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundPayment, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_outbound_payments_id_fail_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_outbound_payments_id_fail_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound payments id post.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundPayment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_outbound_payments_id_post(
        &self,
        args: &PostTestHelpersTreasuryOutboundPaymentsIdPostArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundPayment, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_outbound_payments_id_post_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_outbound_payments_id_post_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound payments id return.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundPayment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_outbound_payments_id_return(
        &self,
        args: &PostTestHelpersTreasuryOutboundPaymentsIdReturnArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundPayment, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_outbound_payments_id_return_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_outbound_payments_id_return_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound transfers outbound transfer.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_outbound_transfers_outbound_transfer(
        &self,
        args: &PostTestHelpersTreasuryOutboundTransfersOutboundTransferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_outbound_transfers_outbound_transfer_builder(
            &self.http_client,
            &args.outbound_transfer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_outbound_transfers_outbound_transfer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound transfers outbound transfer fail.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_outbound_transfers_outbound_transfer_fail(
        &self,
        args: &PostTestHelpersTreasuryOutboundTransfersOutboundTransferFailArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_outbound_transfers_outbound_transfer_fail_builder(
            &self.http_client,
            &args.outbound_transfer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_outbound_transfers_outbound_transfer_fail_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound transfers outbound transfer post.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_outbound_transfers_outbound_transfer_post(
        &self,
        args: &PostTestHelpersTreasuryOutboundTransfersOutboundTransferPostArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_outbound_transfers_outbound_transfer_post_builder(
            &self.http_client,
            &args.outbound_transfer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_outbound_transfers_outbound_transfer_post_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound transfers outbound transfer return.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_outbound_transfers_outbound_transfer_return(
        &self,
        args: &PostTestHelpersTreasuryOutboundTransfersOutboundTransferReturnArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_outbound_transfers_outbound_transfer_return_builder(
            &self.http_client,
            &args.outbound_transfer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_outbound_transfers_outbound_transfer_return_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury received credits.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryReceivedCredit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_received_credits(
        &self,
        args: &PostTestHelpersTreasuryReceivedCreditsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryReceivedCredit, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_received_credits_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_received_credits_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury received debits.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryReceivedDebit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_test_helpers_treasury_received_debits(
        &self,
        args: &PostTestHelpersTreasuryReceivedDebitsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryReceivedDebit, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_test_helpers_treasury_received_debits_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_test_helpers_treasury_received_debits_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tokens.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Token result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_tokens(
        &self,
        args: &PostTokensArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Token, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_tokens_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_tokens_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post topups.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_topups(
        &self,
        args: &PostTopupsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topup, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_topups_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_topups_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post topups topup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_topups_topup(
        &self,
        args: &PostTopupsTopupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topup, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_topups_topup_builder(
            &self.http_client,
            &args.topup,
        )
        .map_err(ProviderError::Api)?;

        let task = post_topups_topup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post topups topup cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_topups_topup_cancel(
        &self,
        args: &PostTopupsTopupCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topup, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_topups_topup_cancel_builder(
            &self.http_client,
            &args.topup,
        )
        .map_err(ProviderError::Api)?;

        let task = post_topups_topup_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post transfers.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Transfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_transfers(
        &self,
        args: &PostTransfersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Transfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_transfers_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_transfers_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post transfers id reversals.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransferReversal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_transfers_id_reversals(
        &self,
        args: &PostTransfersIdReversalsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferReversal, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_transfers_id_reversals_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_transfers_id_reversals_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post transfers transfer.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Transfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_transfers_transfer(
        &self,
        args: &PostTransfersTransferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Transfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_transfers_transfer_builder(
            &self.http_client,
            &args.transfer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_transfers_transfer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post transfers transfer reversals id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransferReversal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_transfers_transfer_reversals_id(
        &self,
        args: &PostTransfersTransferReversalsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferReversal, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_transfers_transfer_reversals_id_builder(
            &self.http_client,
            &args.id,
            &args.transfer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_transfers_transfer_reversals_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury credit reversals.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryCreditReversal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_credit_reversals(
        &self,
        args: &PostTreasuryCreditReversalsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryCreditReversal, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_credit_reversals_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_credit_reversals_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury debit reversals.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryDebitReversal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_debit_reversals(
        &self,
        args: &PostTreasuryDebitReversalsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryDebitReversal, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_debit_reversals_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_debit_reversals_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury financial accounts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryFinancialAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_financial_accounts(
        &self,
        args: &PostTreasuryFinancialAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryFinancialAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_financial_accounts_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_financial_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury financial accounts financial account.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryFinancialAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_financial_accounts_financial_account(
        &self,
        args: &PostTreasuryFinancialAccountsFinancialAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryFinancialAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_financial_accounts_financial_account_builder(
            &self.http_client,
            &args.financial_account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_financial_accounts_financial_account_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury financial accounts financial account close.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryFinancialAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_financial_accounts_financial_account_close(
        &self,
        args: &PostTreasuryFinancialAccountsFinancialAccountCloseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryFinancialAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_financial_accounts_financial_account_close_builder(
            &self.http_client,
            &args.financial_account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_financial_accounts_financial_account_close_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury financial accounts financial account features.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryFinancialAccountFeatures result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_financial_accounts_financial_account_features(
        &self,
        args: &PostTreasuryFinancialAccountsFinancialAccountFeaturesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryFinancialAccountFeatures, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_financial_accounts_financial_account_features_builder(
            &self.http_client,
            &args.financial_account,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_financial_accounts_financial_account_features_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury inbound transfers.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryInboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_inbound_transfers(
        &self,
        args: &PostTreasuryInboundTransfersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryInboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_inbound_transfers_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_inbound_transfers_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury inbound transfers inbound transfer cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryInboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_inbound_transfers_inbound_transfer_cancel(
        &self,
        args: &PostTreasuryInboundTransfersInboundTransferCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryInboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_inbound_transfers_inbound_transfer_cancel_builder(
            &self.http_client,
            &args.inbound_transfer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_inbound_transfers_inbound_transfer_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury outbound payments.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundPayment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_outbound_payments(
        &self,
        args: &PostTreasuryOutboundPaymentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundPayment, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_outbound_payments_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_outbound_payments_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury outbound payments id cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundPayment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_outbound_payments_id_cancel(
        &self,
        args: &PostTreasuryOutboundPaymentsIdCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundPayment, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_outbound_payments_id_cancel_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_outbound_payments_id_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury outbound transfers.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_outbound_transfers(
        &self,
        args: &PostTreasuryOutboundTransfersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_outbound_transfers_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_outbound_transfers_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post treasury outbound transfers outbound transfer cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryOutboundTransfer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_treasury_outbound_transfers_outbound_transfer_cancel(
        &self,
        args: &PostTreasuryOutboundTransfersOutboundTransferCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_treasury_outbound_transfers_outbound_transfer_cancel_builder(
            &self.http_client,
            &args.outbound_transfer,
        )
        .map_err(ProviderError::Api)?;

        let task = post_treasury_outbound_transfers_outbound_transfer_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post webhook endpoints.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebhookEndpoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_webhook_endpoints(
        &self,
        args: &PostWebhookEndpointsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebhookEndpoint, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_webhook_endpoints_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_webhook_endpoints_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post webhook endpoints webhook endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebhookEndpoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_webhook_endpoints_webhook_endpoint(
        &self,
        args: &PostWebhookEndpointsWebhookEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebhookEndpoint, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_webhook_endpoints_webhook_endpoint_builder(
            &self.http_client,
            &args.webhook_endpoint,
        )
        .map_err(ProviderError::Api)?;

        let task = post_webhook_endpoints_webhook_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete webhook endpoints webhook endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeletedWebhookEndpoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_webhook_endpoints_webhook_endpoint(
        &self,
        args: &DeleteWebhookEndpointsWebhookEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedWebhookEndpoint, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_webhook_endpoints_webhook_endpoint_builder(
            &self.http_client,
            &args.webhook_endpoint,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_webhook_endpoints_webhook_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
