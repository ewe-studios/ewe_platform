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

use crate::providers::stripe::clients::{
    get_account_builder, get_account_task,
    post_account_links_builder, post_account_links_task,
    post_account_sessions_builder, post_account_sessions_task,
    get_accounts_builder, get_accounts_task,
    post_accounts_builder, post_accounts_task,
    get_accounts_account_builder, get_accounts_account_task,
    post_accounts_account_builder, post_accounts_account_task,
    delete_accounts_account_builder, delete_accounts_account_task,
    post_accounts_account_bank_accounts_builder, post_accounts_account_bank_accounts_task,
    get_accounts_account_bank_accounts_id_builder, get_accounts_account_bank_accounts_id_task,
    post_accounts_account_bank_accounts_id_builder, post_accounts_account_bank_accounts_id_task,
    delete_accounts_account_bank_accounts_id_builder, delete_accounts_account_bank_accounts_id_task,
    get_accounts_account_capabilities_builder, get_accounts_account_capabilities_task,
    get_accounts_account_capabilities_capability_builder, get_accounts_account_capabilities_capability_task,
    post_accounts_account_capabilities_capability_builder, post_accounts_account_capabilities_capability_task,
    get_accounts_account_external_accounts_builder, get_accounts_account_external_accounts_task,
    post_accounts_account_external_accounts_builder, post_accounts_account_external_accounts_task,
    get_accounts_account_external_accounts_id_builder, get_accounts_account_external_accounts_id_task,
    post_accounts_account_external_accounts_id_builder, post_accounts_account_external_accounts_id_task,
    delete_accounts_account_external_accounts_id_builder, delete_accounts_account_external_accounts_id_task,
    post_accounts_account_login_links_builder, post_accounts_account_login_links_task,
    get_accounts_account_people_builder, get_accounts_account_people_task,
    post_accounts_account_people_builder, post_accounts_account_people_task,
    get_accounts_account_people_person_builder, get_accounts_account_people_person_task,
    post_accounts_account_people_person_builder, post_accounts_account_people_person_task,
    delete_accounts_account_people_person_builder, delete_accounts_account_people_person_task,
    get_accounts_account_persons_builder, get_accounts_account_persons_task,
    post_accounts_account_persons_builder, post_accounts_account_persons_task,
    get_accounts_account_persons_person_builder, get_accounts_account_persons_person_task,
    post_accounts_account_persons_person_builder, post_accounts_account_persons_person_task,
    delete_accounts_account_persons_person_builder, delete_accounts_account_persons_person_task,
    post_accounts_account_reject_builder, post_accounts_account_reject_task,
    get_apple_pay_domains_builder, get_apple_pay_domains_task,
    post_apple_pay_domains_builder, post_apple_pay_domains_task,
    get_apple_pay_domains_domain_builder, get_apple_pay_domains_domain_task,
    delete_apple_pay_domains_domain_builder, delete_apple_pay_domains_domain_task,
    get_application_fees_builder, get_application_fees_task,
    get_application_fees_fee_refunds_id_builder, get_application_fees_fee_refunds_id_task,
    post_application_fees_fee_refunds_id_builder, post_application_fees_fee_refunds_id_task,
    get_application_fees_id_builder, get_application_fees_id_task,
    post_application_fees_id_refund_builder, post_application_fees_id_refund_task,
    get_application_fees_id_refunds_builder, get_application_fees_id_refunds_task,
    post_application_fees_id_refunds_builder, post_application_fees_id_refunds_task,
    get_apps_secrets_builder, get_apps_secrets_task,
    post_apps_secrets_builder, post_apps_secrets_task,
    post_apps_secrets_delete_builder, post_apps_secrets_delete_task,
    get_apps_secrets_find_builder, get_apps_secrets_find_task,
    get_balance_builder, get_balance_task,
    get_balance_history_builder, get_balance_history_task,
    get_balance_history_id_builder, get_balance_history_id_task,
    get_balance_settings_builder, get_balance_settings_task,
    post_balance_settings_builder, post_balance_settings_task,
    get_balance_transactions_builder, get_balance_transactions_task,
    get_balance_transactions_id_builder, get_balance_transactions_id_task,
    get_billing_alerts_builder, get_billing_alerts_task,
    post_billing_alerts_builder, post_billing_alerts_task,
    get_billing_alerts_id_builder, get_billing_alerts_id_task,
    post_billing_alerts_id_activate_builder, post_billing_alerts_id_activate_task,
    post_billing_alerts_id_archive_builder, post_billing_alerts_id_archive_task,
    post_billing_alerts_id_deactivate_builder, post_billing_alerts_id_deactivate_task,
    get_billing_credit_balance_summary_builder, get_billing_credit_balance_summary_task,
    get_billing_credit_balance_transactions_builder, get_billing_credit_balance_transactions_task,
    get_billing_credit_balance_transactions_id_builder, get_billing_credit_balance_transactions_id_task,
    get_billing_credit_grants_builder, get_billing_credit_grants_task,
    post_billing_credit_grants_builder, post_billing_credit_grants_task,
    get_billing_credit_grants_id_builder, get_billing_credit_grants_id_task,
    post_billing_credit_grants_id_builder, post_billing_credit_grants_id_task,
    post_billing_credit_grants_id_expire_builder, post_billing_credit_grants_id_expire_task,
    post_billing_credit_grants_id_void_builder, post_billing_credit_grants_id_void_task,
    post_billing_meter_event_adjustments_builder, post_billing_meter_event_adjustments_task,
    post_billing_meter_events_builder, post_billing_meter_events_task,
    get_billing_meters_builder, get_billing_meters_task,
    post_billing_meters_builder, post_billing_meters_task,
    get_billing_meters_id_builder, get_billing_meters_id_task,
    post_billing_meters_id_builder, post_billing_meters_id_task,
    post_billing_meters_id_deactivate_builder, post_billing_meters_id_deactivate_task,
    get_billing_meters_id_event_summaries_builder, get_billing_meters_id_event_summaries_task,
    post_billing_meters_id_reactivate_builder, post_billing_meters_id_reactivate_task,
    get_billing_portal_configurations_builder, get_billing_portal_configurations_task,
    post_billing_portal_configurations_builder, post_billing_portal_configurations_task,
    get_billing_portal_configurations_configuration_builder, get_billing_portal_configurations_configuration_task,
    post_billing_portal_configurations_configuration_builder, post_billing_portal_configurations_configuration_task,
    post_billing_portal_sessions_builder, post_billing_portal_sessions_task,
    get_charges_builder, get_charges_task,
    post_charges_builder, post_charges_task,
    get_charges_search_builder, get_charges_search_task,
    get_charges_charge_builder, get_charges_charge_task,
    post_charges_charge_builder, post_charges_charge_task,
    post_charges_charge_capture_builder, post_charges_charge_capture_task,
    get_charges_charge_dispute_builder, get_charges_charge_dispute_task,
    post_charges_charge_dispute_builder, post_charges_charge_dispute_task,
    post_charges_charge_dispute_close_builder, post_charges_charge_dispute_close_task,
    post_charges_charge_refund_builder, post_charges_charge_refund_task,
    get_charges_charge_refunds_builder, get_charges_charge_refunds_task,
    post_charges_charge_refunds_builder, post_charges_charge_refunds_task,
    get_charges_charge_refunds_refund_builder, get_charges_charge_refunds_refund_task,
    post_charges_charge_refunds_refund_builder, post_charges_charge_refunds_refund_task,
    get_checkout_sessions_builder, get_checkout_sessions_task,
    post_checkout_sessions_builder, post_checkout_sessions_task,
    get_checkout_sessions_session_builder, get_checkout_sessions_session_task,
    post_checkout_sessions_session_builder, post_checkout_sessions_session_task,
    post_checkout_sessions_session_expire_builder, post_checkout_sessions_session_expire_task,
    get_checkout_sessions_session_line_items_builder, get_checkout_sessions_session_line_items_task,
    get_climate_orders_builder, get_climate_orders_task,
    post_climate_orders_builder, post_climate_orders_task,
    get_climate_orders_order_builder, get_climate_orders_order_task,
    post_climate_orders_order_builder, post_climate_orders_order_task,
    post_climate_orders_order_cancel_builder, post_climate_orders_order_cancel_task,
    get_climate_products_builder, get_climate_products_task,
    get_climate_products_product_builder, get_climate_products_product_task,
    get_climate_suppliers_builder, get_climate_suppliers_task,
    get_climate_suppliers_supplier_builder, get_climate_suppliers_supplier_task,
    get_confirmation_tokens_confirmation_token_builder, get_confirmation_tokens_confirmation_token_task,
    get_country_specs_builder, get_country_specs_task,
    get_country_specs_country_builder, get_country_specs_country_task,
    get_coupons_builder, get_coupons_task,
    post_coupons_builder, post_coupons_task,
    get_coupons_coupon_builder, get_coupons_coupon_task,
    post_coupons_coupon_builder, post_coupons_coupon_task,
    delete_coupons_coupon_builder, delete_coupons_coupon_task,
    get_credit_notes_builder, get_credit_notes_task,
    post_credit_notes_builder, post_credit_notes_task,
    get_credit_notes_preview_builder, get_credit_notes_preview_task,
    get_credit_notes_preview_lines_builder, get_credit_notes_preview_lines_task,
    get_credit_notes_credit_note_lines_builder, get_credit_notes_credit_note_lines_task,
    get_credit_notes_id_builder, get_credit_notes_id_task,
    post_credit_notes_id_builder, post_credit_notes_id_task,
    post_credit_notes_id_void_builder, post_credit_notes_id_void_task,
    post_customer_sessions_builder, post_customer_sessions_task,
    get_customers_builder, get_customers_task,
    post_customers_builder, post_customers_task,
    get_customers_search_builder, get_customers_search_task,
    get_customers_customer_builder, get_customers_customer_task,
    post_customers_customer_builder, post_customers_customer_task,
    delete_customers_customer_builder, delete_customers_customer_task,
    get_customers_customer_balance_transactions_builder, get_customers_customer_balance_transactions_task,
    post_customers_customer_balance_transactions_builder, post_customers_customer_balance_transactions_task,
    get_customers_customer_balance_transactions_transaction_builder, get_customers_customer_balance_transactions_transaction_task,
    post_customers_customer_balance_transactions_transaction_builder, post_customers_customer_balance_transactions_transaction_task,
    get_customers_customer_bank_accounts_builder, get_customers_customer_bank_accounts_task,
    post_customers_customer_bank_accounts_builder, post_customers_customer_bank_accounts_task,
    get_customers_customer_bank_accounts_id_builder, get_customers_customer_bank_accounts_id_task,
    post_customers_customer_bank_accounts_id_builder, post_customers_customer_bank_accounts_id_task,
    delete_customers_customer_bank_accounts_id_builder, delete_customers_customer_bank_accounts_id_task,
    post_customers_customer_bank_accounts_id_verify_builder, post_customers_customer_bank_accounts_id_verify_task,
    get_customers_customer_cards_builder, get_customers_customer_cards_task,
    post_customers_customer_cards_builder, post_customers_customer_cards_task,
    get_customers_customer_cards_id_builder, get_customers_customer_cards_id_task,
    post_customers_customer_cards_id_builder, post_customers_customer_cards_id_task,
    delete_customers_customer_cards_id_builder, delete_customers_customer_cards_id_task,
    get_customers_customer_cash_balance_builder, get_customers_customer_cash_balance_task,
    post_customers_customer_cash_balance_builder, post_customers_customer_cash_balance_task,
    get_customers_customer_cash_balance_transactions_builder, get_customers_customer_cash_balance_transactions_task,
    get_customers_customer_cash_balance_transactions_transaction_builder, get_customers_customer_cash_balance_transactions_transaction_task,
    get_customers_customer_discount_builder, get_customers_customer_discount_task,
    delete_customers_customer_discount_builder, delete_customers_customer_discount_task,
    post_customers_customer_funding_instructions_builder, post_customers_customer_funding_instructions_task,
    get_customers_customer_payment_methods_builder, get_customers_customer_payment_methods_task,
    get_customers_customer_payment_methods_payment_method_builder, get_customers_customer_payment_methods_payment_method_task,
    get_customers_customer_sources_builder, get_customers_customer_sources_task,
    post_customers_customer_sources_builder, post_customers_customer_sources_task,
    get_customers_customer_sources_id_builder, get_customers_customer_sources_id_task,
    post_customers_customer_sources_id_builder, post_customers_customer_sources_id_task,
    delete_customers_customer_sources_id_builder, delete_customers_customer_sources_id_task,
    post_customers_customer_sources_id_verify_builder, post_customers_customer_sources_id_verify_task,
    get_customers_customer_subscriptions_builder, get_customers_customer_subscriptions_task,
    post_customers_customer_subscriptions_builder, post_customers_customer_subscriptions_task,
    get_customers_customer_subscriptions_subscription_exposed_id_builder, get_customers_customer_subscriptions_subscription_exposed_id_task,
    post_customers_customer_subscriptions_subscription_exposed_id_builder, post_customers_customer_subscriptions_subscription_exposed_id_task,
    delete_customers_customer_subscriptions_subscription_exposed_id_builder, delete_customers_customer_subscriptions_subscription_exposed_id_task,
    get_customers_customer_subscriptions_subscription_exposed_id_discount_builder, get_customers_customer_subscriptions_subscription_exposed_id_discount_task,
    delete_customers_customer_subscriptions_subscription_exposed_id_discount_builder, delete_customers_customer_subscriptions_subscription_exposed_id_discount_task,
    get_customers_customer_tax_ids_builder, get_customers_customer_tax_ids_task,
    post_customers_customer_tax_ids_builder, post_customers_customer_tax_ids_task,
    get_customers_customer_tax_ids_id_builder, get_customers_customer_tax_ids_id_task,
    delete_customers_customer_tax_ids_id_builder, delete_customers_customer_tax_ids_id_task,
    get_disputes_builder, get_disputes_task,
    get_disputes_dispute_builder, get_disputes_dispute_task,
    post_disputes_dispute_builder, post_disputes_dispute_task,
    post_disputes_dispute_close_builder, post_disputes_dispute_close_task,
    get_entitlements_active_entitlements_builder, get_entitlements_active_entitlements_task,
    get_entitlements_active_entitlements_id_builder, get_entitlements_active_entitlements_id_task,
    get_entitlements_features_builder, get_entitlements_features_task,
    post_entitlements_features_builder, post_entitlements_features_task,
    get_entitlements_features_id_builder, get_entitlements_features_id_task,
    post_entitlements_features_id_builder, post_entitlements_features_id_task,
    post_ephemeral_keys_builder, post_ephemeral_keys_task,
    delete_ephemeral_keys_key_builder, delete_ephemeral_keys_key_task,
    get_events_builder, get_events_task,
    get_events_id_builder, get_events_id_task,
    get_exchange_rates_builder, get_exchange_rates_task,
    get_exchange_rates_rate_id_builder, get_exchange_rates_rate_id_task,
    post_external_accounts_id_builder, post_external_accounts_id_task,
    get_file_links_builder, get_file_links_task,
    post_file_links_builder, post_file_links_task,
    get_file_links_link_builder, get_file_links_link_task,
    post_file_links_link_builder, post_file_links_link_task,
    get_files_builder, get_files_task,
    post_files_builder, post_files_task,
    get_files_file_builder, get_files_file_task,
    get_financial_connections_accounts_builder, get_financial_connections_accounts_task,
    get_financial_connections_accounts_account_builder, get_financial_connections_accounts_account_task,
    post_financial_connections_accounts_account_disconnect_builder, post_financial_connections_accounts_account_disconnect_task,
    get_financial_connections_accounts_account_owners_builder, get_financial_connections_accounts_account_owners_task,
    post_financial_connections_accounts_account_refresh_builder, post_financial_connections_accounts_account_refresh_task,
    post_financial_connections_accounts_account_subscribe_builder, post_financial_connections_accounts_account_subscribe_task,
    post_financial_connections_accounts_account_unsubscribe_builder, post_financial_connections_accounts_account_unsubscribe_task,
    post_financial_connections_sessions_builder, post_financial_connections_sessions_task,
    get_financial_connections_sessions_session_builder, get_financial_connections_sessions_session_task,
    get_financial_connections_transactions_builder, get_financial_connections_transactions_task,
    get_financial_connections_transactions_transaction_builder, get_financial_connections_transactions_transaction_task,
    get_forwarding_requests_builder, get_forwarding_requests_task,
    post_forwarding_requests_builder, post_forwarding_requests_task,
    get_forwarding_requests_id_builder, get_forwarding_requests_id_task,
    get_identity_verification_reports_builder, get_identity_verification_reports_task,
    get_identity_verification_reports_report_builder, get_identity_verification_reports_report_task,
    get_identity_verification_sessions_builder, get_identity_verification_sessions_task,
    post_identity_verification_sessions_builder, post_identity_verification_sessions_task,
    get_identity_verification_sessions_session_builder, get_identity_verification_sessions_session_task,
    post_identity_verification_sessions_session_builder, post_identity_verification_sessions_session_task,
    post_identity_verification_sessions_session_cancel_builder, post_identity_verification_sessions_session_cancel_task,
    post_identity_verification_sessions_session_redact_builder, post_identity_verification_sessions_session_redact_task,
    get_invoice_payments_builder, get_invoice_payments_task,
    get_invoice_payments_invoice_payment_builder, get_invoice_payments_invoice_payment_task,
    get_invoice_rendering_templates_builder, get_invoice_rendering_templates_task,
    get_invoice_rendering_templates_template_builder, get_invoice_rendering_templates_template_task,
    post_invoice_rendering_templates_template_archive_builder, post_invoice_rendering_templates_template_archive_task,
    post_invoice_rendering_templates_template_unarchive_builder, post_invoice_rendering_templates_template_unarchive_task,
    get_invoiceitems_builder, get_invoiceitems_task,
    post_invoiceitems_builder, post_invoiceitems_task,
    get_invoiceitems_invoiceitem_builder, get_invoiceitems_invoiceitem_task,
    post_invoiceitems_invoiceitem_builder, post_invoiceitems_invoiceitem_task,
    delete_invoiceitems_invoiceitem_builder, delete_invoiceitems_invoiceitem_task,
    get_invoices_builder, get_invoices_task,
    post_invoices_builder, post_invoices_task,
    post_invoices_create_preview_builder, post_invoices_create_preview_task,
    get_invoices_search_builder, get_invoices_search_task,
    get_invoices_invoice_builder, get_invoices_invoice_task,
    post_invoices_invoice_builder, post_invoices_invoice_task,
    delete_invoices_invoice_builder, delete_invoices_invoice_task,
    post_invoices_invoice_add_lines_builder, post_invoices_invoice_add_lines_task,
    post_invoices_invoice_attach_payment_builder, post_invoices_invoice_attach_payment_task,
    post_invoices_invoice_finalize_builder, post_invoices_invoice_finalize_task,
    get_invoices_invoice_lines_builder, get_invoices_invoice_lines_task,
    post_invoices_invoice_lines_line_item_id_builder, post_invoices_invoice_lines_line_item_id_task,
    post_invoices_invoice_mark_uncollectible_builder, post_invoices_invoice_mark_uncollectible_task,
    post_invoices_invoice_pay_builder, post_invoices_invoice_pay_task,
    post_invoices_invoice_remove_lines_builder, post_invoices_invoice_remove_lines_task,
    post_invoices_invoice_send_builder, post_invoices_invoice_send_task,
    post_invoices_invoice_update_lines_builder, post_invoices_invoice_update_lines_task,
    post_invoices_invoice_void_builder, post_invoices_invoice_void_task,
    get_issuing_authorizations_builder, get_issuing_authorizations_task,
    get_issuing_authorizations_authorization_builder, get_issuing_authorizations_authorization_task,
    post_issuing_authorizations_authorization_builder, post_issuing_authorizations_authorization_task,
    post_issuing_authorizations_authorization_approve_builder, post_issuing_authorizations_authorization_approve_task,
    post_issuing_authorizations_authorization_decline_builder, post_issuing_authorizations_authorization_decline_task,
    get_issuing_cardholders_builder, get_issuing_cardholders_task,
    post_issuing_cardholders_builder, post_issuing_cardholders_task,
    get_issuing_cardholders_cardholder_builder, get_issuing_cardholders_cardholder_task,
    post_issuing_cardholders_cardholder_builder, post_issuing_cardholders_cardholder_task,
    get_issuing_cards_builder, get_issuing_cards_task,
    post_issuing_cards_builder, post_issuing_cards_task,
    get_issuing_cards_card_builder, get_issuing_cards_card_task,
    post_issuing_cards_card_builder, post_issuing_cards_card_task,
    get_issuing_disputes_builder, get_issuing_disputes_task,
    post_issuing_disputes_builder, post_issuing_disputes_task,
    get_issuing_disputes_dispute_builder, get_issuing_disputes_dispute_task,
    post_issuing_disputes_dispute_builder, post_issuing_disputes_dispute_task,
    post_issuing_disputes_dispute_submit_builder, post_issuing_disputes_dispute_submit_task,
    get_issuing_personalization_designs_builder, get_issuing_personalization_designs_task,
    post_issuing_personalization_designs_builder, post_issuing_personalization_designs_task,
    get_issuing_personalization_designs_personalization_design_builder, get_issuing_personalization_designs_personalization_design_task,
    post_issuing_personalization_designs_personalization_design_builder, post_issuing_personalization_designs_personalization_design_task,
    get_issuing_physical_bundles_builder, get_issuing_physical_bundles_task,
    get_issuing_physical_bundles_physical_bundle_builder, get_issuing_physical_bundles_physical_bundle_task,
    get_issuing_settlements_settlement_builder, get_issuing_settlements_settlement_task,
    post_issuing_settlements_settlement_builder, post_issuing_settlements_settlement_task,
    get_issuing_tokens_builder, get_issuing_tokens_task,
    get_issuing_tokens_token_builder, get_issuing_tokens_token_task,
    post_issuing_tokens_token_builder, post_issuing_tokens_token_task,
    get_issuing_transactions_builder, get_issuing_transactions_task,
    get_issuing_transactions_transaction_builder, get_issuing_transactions_transaction_task,
    post_issuing_transactions_transaction_builder, post_issuing_transactions_transaction_task,
    post_link_account_sessions_builder, post_link_account_sessions_task,
    get_link_account_sessions_session_builder, get_link_account_sessions_session_task,
    get_linked_accounts_builder, get_linked_accounts_task,
    get_linked_accounts_account_builder, get_linked_accounts_account_task,
    post_linked_accounts_account_disconnect_builder, post_linked_accounts_account_disconnect_task,
    get_linked_accounts_account_owners_builder, get_linked_accounts_account_owners_task,
    post_linked_accounts_account_refresh_builder, post_linked_accounts_account_refresh_task,
    get_mandates_mandate_builder, get_mandates_mandate_task,
    get_payment_attempt_records_builder, get_payment_attempt_records_task,
    get_payment_attempt_records_id_builder, get_payment_attempt_records_id_task,
    get_payment_intents_builder, get_payment_intents_task,
    post_payment_intents_builder, post_payment_intents_task,
    get_payment_intents_search_builder, get_payment_intents_search_task,
    get_payment_intents_intent_builder, get_payment_intents_intent_task,
    post_payment_intents_intent_builder, post_payment_intents_intent_task,
    get_payment_intents_intent_amount_details_line_items_builder, get_payment_intents_intent_amount_details_line_items_task,
    post_payment_intents_intent_apply_customer_balance_builder, post_payment_intents_intent_apply_customer_balance_task,
    post_payment_intents_intent_cancel_builder, post_payment_intents_intent_cancel_task,
    post_payment_intents_intent_capture_builder, post_payment_intents_intent_capture_task,
    post_payment_intents_intent_confirm_builder, post_payment_intents_intent_confirm_task,
    post_payment_intents_intent_increment_authorization_builder, post_payment_intents_intent_increment_authorization_task,
    post_payment_intents_intent_verify_microdeposits_builder, post_payment_intents_intent_verify_microdeposits_task,
    get_payment_links_builder, get_payment_links_task,
    post_payment_links_builder, post_payment_links_task,
    get_payment_links_payment_link_builder, get_payment_links_payment_link_task,
    post_payment_links_payment_link_builder, post_payment_links_payment_link_task,
    get_payment_links_payment_link_line_items_builder, get_payment_links_payment_link_line_items_task,
    get_payment_method_configurations_builder, get_payment_method_configurations_task,
    post_payment_method_configurations_builder, post_payment_method_configurations_task,
    get_payment_method_configurations_configuration_builder, get_payment_method_configurations_configuration_task,
    post_payment_method_configurations_configuration_builder, post_payment_method_configurations_configuration_task,
    get_payment_method_domains_builder, get_payment_method_domains_task,
    post_payment_method_domains_builder, post_payment_method_domains_task,
    get_payment_method_domains_payment_method_domain_builder, get_payment_method_domains_payment_method_domain_task,
    post_payment_method_domains_payment_method_domain_builder, post_payment_method_domains_payment_method_domain_task,
    post_payment_method_domains_payment_method_domain_validate_builder, post_payment_method_domains_payment_method_domain_validate_task,
    get_payment_methods_builder, get_payment_methods_task,
    post_payment_methods_builder, post_payment_methods_task,
    get_payment_methods_payment_method_builder, get_payment_methods_payment_method_task,
    post_payment_methods_payment_method_builder, post_payment_methods_payment_method_task,
    post_payment_methods_payment_method_attach_builder, post_payment_methods_payment_method_attach_task,
    post_payment_methods_payment_method_detach_builder, post_payment_methods_payment_method_detach_task,
    post_payment_records_report_payment_builder, post_payment_records_report_payment_task,
    get_payment_records_id_builder, get_payment_records_id_task,
    post_payment_records_id_report_payment_attempt_builder, post_payment_records_id_report_payment_attempt_task,
    post_payment_records_id_report_payment_attempt_canceled_builder, post_payment_records_id_report_payment_attempt_canceled_task,
    post_payment_records_id_report_payment_attempt_failed_builder, post_payment_records_id_report_payment_attempt_failed_task,
    post_payment_records_id_report_payment_attempt_guaranteed_builder, post_payment_records_id_report_payment_attempt_guaranteed_task,
    post_payment_records_id_report_payment_attempt_informational_builder, post_payment_records_id_report_payment_attempt_informational_task,
    post_payment_records_id_report_refund_builder, post_payment_records_id_report_refund_task,
    get_payouts_builder, get_payouts_task,
    post_payouts_builder, post_payouts_task,
    get_payouts_payout_builder, get_payouts_payout_task,
    post_payouts_payout_builder, post_payouts_payout_task,
    post_payouts_payout_cancel_builder, post_payouts_payout_cancel_task,
    post_payouts_payout_reverse_builder, post_payouts_payout_reverse_task,
    get_plans_builder, get_plans_task,
    post_plans_builder, post_plans_task,
    get_plans_plan_builder, get_plans_plan_task,
    post_plans_plan_builder, post_plans_plan_task,
    delete_plans_plan_builder, delete_plans_plan_task,
    get_prices_builder, get_prices_task,
    post_prices_builder, post_prices_task,
    get_prices_search_builder, get_prices_search_task,
    get_prices_price_builder, get_prices_price_task,
    post_prices_price_builder, post_prices_price_task,
    get_products_builder, get_products_task,
    post_products_builder, post_products_task,
    get_products_search_builder, get_products_search_task,
    get_products_id_builder, get_products_id_task,
    post_products_id_builder, post_products_id_task,
    delete_products_id_builder, delete_products_id_task,
    get_products_product_features_builder, get_products_product_features_task,
    post_products_product_features_builder, post_products_product_features_task,
    get_products_product_features_id_builder, get_products_product_features_id_task,
    delete_products_product_features_id_builder, delete_products_product_features_id_task,
    get_promotion_codes_builder, get_promotion_codes_task,
    post_promotion_codes_builder, post_promotion_codes_task,
    get_promotion_codes_promotion_code_builder, get_promotion_codes_promotion_code_task,
    post_promotion_codes_promotion_code_builder, post_promotion_codes_promotion_code_task,
    get_quotes_builder, get_quotes_task,
    post_quotes_builder, post_quotes_task,
    get_quotes_quote_builder, get_quotes_quote_task,
    post_quotes_quote_builder, post_quotes_quote_task,
    post_quotes_quote_accept_builder, post_quotes_quote_accept_task,
    post_quotes_quote_cancel_builder, post_quotes_quote_cancel_task,
    get_quotes_quote_computed_upfront_line_items_builder, get_quotes_quote_computed_upfront_line_items_task,
    post_quotes_quote_finalize_builder, post_quotes_quote_finalize_task,
    get_quotes_quote_line_items_builder, get_quotes_quote_line_items_task,
    get_quotes_quote_pdf_builder, get_quotes_quote_pdf_task,
    get_radar_early_fraud_warnings_builder, get_radar_early_fraud_warnings_task,
    get_radar_early_fraud_warnings_early_fraud_warning_builder, get_radar_early_fraud_warnings_early_fraud_warning_task,
    post_radar_payment_evaluations_builder, post_radar_payment_evaluations_task,
    get_radar_value_list_items_builder, get_radar_value_list_items_task,
    post_radar_value_list_items_builder, post_radar_value_list_items_task,
    get_radar_value_list_items_item_builder, get_radar_value_list_items_item_task,
    delete_radar_value_list_items_item_builder, delete_radar_value_list_items_item_task,
    get_radar_value_lists_builder, get_radar_value_lists_task,
    post_radar_value_lists_builder, post_radar_value_lists_task,
    get_radar_value_lists_value_list_builder, get_radar_value_lists_value_list_task,
    post_radar_value_lists_value_list_builder, post_radar_value_lists_value_list_task,
    delete_radar_value_lists_value_list_builder, delete_radar_value_lists_value_list_task,
    get_refunds_builder, get_refunds_task,
    post_refunds_builder, post_refunds_task,
    get_refunds_refund_builder, get_refunds_refund_task,
    post_refunds_refund_builder, post_refunds_refund_task,
    post_refunds_refund_cancel_builder, post_refunds_refund_cancel_task,
    get_reporting_report_runs_builder, get_reporting_report_runs_task,
    post_reporting_report_runs_builder, post_reporting_report_runs_task,
    get_reporting_report_runs_report_run_builder, get_reporting_report_runs_report_run_task,
    get_reporting_report_types_builder, get_reporting_report_types_task,
    get_reporting_report_types_report_type_builder, get_reporting_report_types_report_type_task,
    get_reviews_builder, get_reviews_task,
    get_reviews_review_builder, get_reviews_review_task,
    post_reviews_review_approve_builder, post_reviews_review_approve_task,
    get_setup_attempts_builder, get_setup_attempts_task,
    get_setup_intents_builder, get_setup_intents_task,
    post_setup_intents_builder, post_setup_intents_task,
    get_setup_intents_intent_builder, get_setup_intents_intent_task,
    post_setup_intents_intent_builder, post_setup_intents_intent_task,
    post_setup_intents_intent_cancel_builder, post_setup_intents_intent_cancel_task,
    post_setup_intents_intent_confirm_builder, post_setup_intents_intent_confirm_task,
    post_setup_intents_intent_verify_microdeposits_builder, post_setup_intents_intent_verify_microdeposits_task,
    get_shipping_rates_builder, get_shipping_rates_task,
    post_shipping_rates_builder, post_shipping_rates_task,
    get_shipping_rates_shipping_rate_token_builder, get_shipping_rates_shipping_rate_token_task,
    post_shipping_rates_shipping_rate_token_builder, post_shipping_rates_shipping_rate_token_task,
    post_sigma_saved_queries_id_builder, post_sigma_saved_queries_id_task,
    get_sigma_scheduled_query_runs_builder, get_sigma_scheduled_query_runs_task,
    get_sigma_scheduled_query_runs_scheduled_query_run_builder, get_sigma_scheduled_query_runs_scheduled_query_run_task,
    post_sources_builder, post_sources_task,
    get_sources_source_builder, get_sources_source_task,
    post_sources_source_builder, post_sources_source_task,
    get_sources_source_mandate_notifications_mandate_notification_builder, get_sources_source_mandate_notifications_mandate_notification_task,
    get_sources_source_source_transactions_builder, get_sources_source_source_transactions_task,
    get_sources_source_source_transactions_source_transaction_builder, get_sources_source_source_transactions_source_transaction_task,
    post_sources_source_verify_builder, post_sources_source_verify_task,
    get_subscription_items_builder, get_subscription_items_task,
    post_subscription_items_builder, post_subscription_items_task,
    get_subscription_items_item_builder, get_subscription_items_item_task,
    post_subscription_items_item_builder, post_subscription_items_item_task,
    delete_subscription_items_item_builder, delete_subscription_items_item_task,
    get_subscription_schedules_builder, get_subscription_schedules_task,
    post_subscription_schedules_builder, post_subscription_schedules_task,
    get_subscription_schedules_schedule_builder, get_subscription_schedules_schedule_task,
    post_subscription_schedules_schedule_builder, post_subscription_schedules_schedule_task,
    post_subscription_schedules_schedule_cancel_builder, post_subscription_schedules_schedule_cancel_task,
    post_subscription_schedules_schedule_release_builder, post_subscription_schedules_schedule_release_task,
    get_subscriptions_builder, get_subscriptions_task,
    post_subscriptions_builder, post_subscriptions_task,
    get_subscriptions_search_builder, get_subscriptions_search_task,
    get_subscriptions_subscription_exposed_id_builder, get_subscriptions_subscription_exposed_id_task,
    post_subscriptions_subscription_exposed_id_builder, post_subscriptions_subscription_exposed_id_task,
    delete_subscriptions_subscription_exposed_id_builder, delete_subscriptions_subscription_exposed_id_task,
    delete_subscriptions_subscription_exposed_id_discount_builder, delete_subscriptions_subscription_exposed_id_discount_task,
    post_subscriptions_subscription_migrate_builder, post_subscriptions_subscription_migrate_task,
    post_subscriptions_subscription_resume_builder, post_subscriptions_subscription_resume_task,
    get_tax_associations_find_builder, get_tax_associations_find_task,
    post_tax_calculations_builder, post_tax_calculations_task,
    get_tax_calculations_calculation_builder, get_tax_calculations_calculation_task,
    get_tax_calculations_calculation_line_items_builder, get_tax_calculations_calculation_line_items_task,
    get_tax_registrations_builder, get_tax_registrations_task,
    post_tax_registrations_builder, post_tax_registrations_task,
    get_tax_registrations_id_builder, get_tax_registrations_id_task,
    post_tax_registrations_id_builder, post_tax_registrations_id_task,
    get_tax_settings_builder, get_tax_settings_task,
    post_tax_settings_builder, post_tax_settings_task,
    post_tax_transactions_create_from_calculation_builder, post_tax_transactions_create_from_calculation_task,
    post_tax_transactions_create_reversal_builder, post_tax_transactions_create_reversal_task,
    get_tax_transactions_transaction_builder, get_tax_transactions_transaction_task,
    get_tax_transactions_transaction_line_items_builder, get_tax_transactions_transaction_line_items_task,
    get_tax_codes_builder, get_tax_codes_task,
    get_tax_codes_id_builder, get_tax_codes_id_task,
    get_tax_ids_builder, get_tax_ids_task,
    post_tax_ids_builder, post_tax_ids_task,
    get_tax_ids_id_builder, get_tax_ids_id_task,
    delete_tax_ids_id_builder, delete_tax_ids_id_task,
    get_tax_rates_builder, get_tax_rates_task,
    post_tax_rates_builder, post_tax_rates_task,
    get_tax_rates_tax_rate_builder, get_tax_rates_tax_rate_task,
    post_tax_rates_tax_rate_builder, post_tax_rates_tax_rate_task,
    get_terminal_configurations_builder, get_terminal_configurations_task,
    post_terminal_configurations_builder, post_terminal_configurations_task,
    get_terminal_configurations_configuration_builder, get_terminal_configurations_configuration_task,
    post_terminal_configurations_configuration_builder, post_terminal_configurations_configuration_task,
    delete_terminal_configurations_configuration_builder, delete_terminal_configurations_configuration_task,
    post_terminal_connection_tokens_builder, post_terminal_connection_tokens_task,
    get_terminal_locations_builder, get_terminal_locations_task,
    post_terminal_locations_builder, post_terminal_locations_task,
    get_terminal_locations_location_builder, get_terminal_locations_location_task,
    post_terminal_locations_location_builder, post_terminal_locations_location_task,
    delete_terminal_locations_location_builder, delete_terminal_locations_location_task,
    post_terminal_onboarding_links_builder, post_terminal_onboarding_links_task,
    get_terminal_readers_builder, get_terminal_readers_task,
    post_terminal_readers_builder, post_terminal_readers_task,
    get_terminal_readers_reader_builder, get_terminal_readers_reader_task,
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
    get_test_helpers_test_clocks_builder, get_test_helpers_test_clocks_task,
    post_test_helpers_test_clocks_builder, post_test_helpers_test_clocks_task,
    get_test_helpers_test_clocks_test_clock_builder, get_test_helpers_test_clocks_test_clock_task,
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
    get_tokens_token_builder, get_tokens_token_task,
    get_topups_builder, get_topups_task,
    post_topups_builder, post_topups_task,
    get_topups_topup_builder, get_topups_topup_task,
    post_topups_topup_builder, post_topups_topup_task,
    post_topups_topup_cancel_builder, post_topups_topup_cancel_task,
    get_transfers_builder, get_transfers_task,
    post_transfers_builder, post_transfers_task,
    get_transfers_id_reversals_builder, get_transfers_id_reversals_task,
    post_transfers_id_reversals_builder, post_transfers_id_reversals_task,
    get_transfers_transfer_builder, get_transfers_transfer_task,
    post_transfers_transfer_builder, post_transfers_transfer_task,
    get_transfers_transfer_reversals_id_builder, get_transfers_transfer_reversals_id_task,
    post_transfers_transfer_reversals_id_builder, post_transfers_transfer_reversals_id_task,
    get_treasury_credit_reversals_builder, get_treasury_credit_reversals_task,
    post_treasury_credit_reversals_builder, post_treasury_credit_reversals_task,
    get_treasury_credit_reversals_credit_reversal_builder, get_treasury_credit_reversals_credit_reversal_task,
    get_treasury_debit_reversals_builder, get_treasury_debit_reversals_task,
    post_treasury_debit_reversals_builder, post_treasury_debit_reversals_task,
    get_treasury_debit_reversals_debit_reversal_builder, get_treasury_debit_reversals_debit_reversal_task,
    get_treasury_financial_accounts_builder, get_treasury_financial_accounts_task,
    post_treasury_financial_accounts_builder, post_treasury_financial_accounts_task,
    get_treasury_financial_accounts_financial_account_builder, get_treasury_financial_accounts_financial_account_task,
    post_treasury_financial_accounts_financial_account_builder, post_treasury_financial_accounts_financial_account_task,
    post_treasury_financial_accounts_financial_account_close_builder, post_treasury_financial_accounts_financial_account_close_task,
    get_treasury_financial_accounts_financial_account_features_builder, get_treasury_financial_accounts_financial_account_features_task,
    post_treasury_financial_accounts_financial_account_features_builder, post_treasury_financial_accounts_financial_account_features_task,
    get_treasury_inbound_transfers_builder, get_treasury_inbound_transfers_task,
    post_treasury_inbound_transfers_builder, post_treasury_inbound_transfers_task,
    get_treasury_inbound_transfers_id_builder, get_treasury_inbound_transfers_id_task,
    post_treasury_inbound_transfers_inbound_transfer_cancel_builder, post_treasury_inbound_transfers_inbound_transfer_cancel_task,
    get_treasury_outbound_payments_builder, get_treasury_outbound_payments_task,
    post_treasury_outbound_payments_builder, post_treasury_outbound_payments_task,
    get_treasury_outbound_payments_id_builder, get_treasury_outbound_payments_id_task,
    post_treasury_outbound_payments_id_cancel_builder, post_treasury_outbound_payments_id_cancel_task,
    get_treasury_outbound_transfers_builder, get_treasury_outbound_transfers_task,
    post_treasury_outbound_transfers_builder, post_treasury_outbound_transfers_task,
    get_treasury_outbound_transfers_outbound_transfer_builder, get_treasury_outbound_transfers_outbound_transfer_task,
    post_treasury_outbound_transfers_outbound_transfer_cancel_builder, post_treasury_outbound_transfers_outbound_transfer_cancel_task,
    get_treasury_received_credits_builder, get_treasury_received_credits_task,
    get_treasury_received_credits_id_builder, get_treasury_received_credits_id_task,
    get_treasury_received_debits_builder, get_treasury_received_debits_task,
    get_treasury_received_debits_id_builder, get_treasury_received_debits_id_task,
    get_treasury_transaction_entries_builder, get_treasury_transaction_entries_task,
    get_treasury_transaction_entries_id_builder, get_treasury_transaction_entries_id_task,
    get_treasury_transactions_builder, get_treasury_transactions_task,
    get_treasury_transactions_id_builder, get_treasury_transactions_id_task,
    get_webhook_endpoints_builder, get_webhook_endpoints_task,
    post_webhook_endpoints_builder, post_webhook_endpoints_task,
    get_webhook_endpoints_webhook_endpoint_builder, get_webhook_endpoints_webhook_endpoint_task,
    post_webhook_endpoints_webhook_endpoint_builder, post_webhook_endpoints_webhook_endpoint_task,
    delete_webhook_endpoints_webhook_endpoint_builder, delete_webhook_endpoints_webhook_endpoint_task,
};
use crate::providers::stripe::clients::types::{ApiError, ApiPending};
use crate::providers::stripe::clients::Account;
use crate::providers::stripe::clients::AccountLink;
use crate::providers::stripe::clients::AccountSession;
use crate::providers::stripe::clients::ApplePayDomain;
use crate::providers::stripe::clients::ApplicationFee;
use crate::providers::stripe::clients::AppsSecret;
use crate::providers::stripe::clients::Balance;
use crate::providers::stripe::clients::BalanceSettings;
use crate::providers::stripe::clients::BalanceTransaction;
use crate::providers::stripe::clients::BankAccount;
use crate::providers::stripe::clients::BillingAlert;
use crate::providers::stripe::clients::BillingCreditBalanceSummary;
use crate::providers::stripe::clients::BillingCreditBalanceTransaction;
use crate::providers::stripe::clients::BillingCreditGrant;
use crate::providers::stripe::clients::BillingMeter;
use crate::providers::stripe::clients::BillingMeterEvent;
use crate::providers::stripe::clients::BillingMeterEventAdjustment;
use crate::providers::stripe::clients::BillingPortalConfiguration;
use crate::providers::stripe::clients::BillingPortalSession;
use crate::providers::stripe::clients::Capability;
use crate::providers::stripe::clients::Card;
use crate::providers::stripe::clients::CashBalance;
use crate::providers::stripe::clients::Charge;
use crate::providers::stripe::clients::CheckoutSession;
use crate::providers::stripe::clients::ClimateOrder;
use crate::providers::stripe::clients::ClimateProduct;
use crate::providers::stripe::clients::ClimateSupplier;
use crate::providers::stripe::clients::ConfirmationToken;
use crate::providers::stripe::clients::CountrySpec;
use crate::providers::stripe::clients::Coupon;
use crate::providers::stripe::clients::CreditNote;
use crate::providers::stripe::clients::Customer;
use crate::providers::stripe::clients::CustomerBalanceTransaction;
use crate::providers::stripe::clients::CustomerCashBalanceTransaction;
use crate::providers::stripe::clients::CustomerSession;
use crate::providers::stripe::clients::DeletedAccount;
use crate::providers::stripe::clients::DeletedApplePayDomain;
use crate::providers::stripe::clients::DeletedCoupon;
use crate::providers::stripe::clients::DeletedCustomer;
use crate::providers::stripe::clients::DeletedDiscount;
use crate::providers::stripe::clients::DeletedExternalAccount;
use crate::providers::stripe::clients::DeletedInvoice;
use crate::providers::stripe::clients::DeletedInvoiceitem;
use crate::providers::stripe::clients::DeletedPerson;
use crate::providers::stripe::clients::DeletedPlan;
use crate::providers::stripe::clients::DeletedProduct;
use crate::providers::stripe::clients::DeletedProductFeature;
use crate::providers::stripe::clients::DeletedRadarValueList;
use crate::providers::stripe::clients::DeletedRadarValueListItem;
use crate::providers::stripe::clients::DeletedSubscriptionItem;
use crate::providers::stripe::clients::DeletedTaxId;
use crate::providers::stripe::clients::DeletedTerminalConfiguration;
use crate::providers::stripe::clients::DeletedTerminalLocation;
use crate::providers::stripe::clients::DeletedTerminalReader;
use crate::providers::stripe::clients::DeletedTestHelpersTestClock;
use crate::providers::stripe::clients::DeletedWebhookEndpoint;
use crate::providers::stripe::clients::Discount;
use crate::providers::stripe::clients::Dispute;
use crate::providers::stripe::clients::EntitlementsActiveEntitlement;
use crate::providers::stripe::clients::EntitlementsFeature;
use crate::providers::stripe::clients::EphemeralKey;
use crate::providers::stripe::clients::Event;
use crate::providers::stripe::clients::ExchangeRate;
use crate::providers::stripe::clients::ExternalAccount;
use crate::providers::stripe::clients::FeeRefund;
use crate::providers::stripe::clients::File;
use crate::providers::stripe::clients::FileLink;
use crate::providers::stripe::clients::FinancialConnectionsAccount;
use crate::providers::stripe::clients::FinancialConnectionsSession;
use crate::providers::stripe::clients::FinancialConnectionsTransaction;
use crate::providers::stripe::clients::ForwardingRequest;
use crate::providers::stripe::clients::FundingInstructions;
use crate::providers::stripe::clients::IdentityVerificationReport;
use crate::providers::stripe::clients::IdentityVerificationSession;
use crate::providers::stripe::clients::Invoice;
use crate::providers::stripe::clients::InvoicePayment;
use crate::providers::stripe::clients::InvoiceRenderingTemplate;
use crate::providers::stripe::clients::Invoiceitem;
use crate::providers::stripe::clients::IssuingAuthorization;
use crate::providers::stripe::clients::IssuingCard;
use crate::providers::stripe::clients::IssuingCardholder;
use crate::providers::stripe::clients::IssuingDispute;
use crate::providers::stripe::clients::IssuingPersonalizationDesign;
use crate::providers::stripe::clients::IssuingPhysicalBundle;
use crate::providers::stripe::clients::IssuingSettlement;
use crate::providers::stripe::clients::IssuingToken;
use crate::providers::stripe::clients::IssuingTransaction;
use crate::providers::stripe::clients::LineItem;
use crate::providers::stripe::clients::LoginLink;
use crate::providers::stripe::clients::Mandate;
use crate::providers::stripe::clients::PaymentAttemptRecord;
use crate::providers::stripe::clients::PaymentIntent;
use crate::providers::stripe::clients::PaymentLink;
use crate::providers::stripe::clients::PaymentMethod;
use crate::providers::stripe::clients::PaymentMethodConfiguration;
use crate::providers::stripe::clients::PaymentMethodDomain;
use crate::providers::stripe::clients::PaymentRecord;
use crate::providers::stripe::clients::PaymentSource;
use crate::providers::stripe::clients::Payout;
use crate::providers::stripe::clients::Person;
use crate::providers::stripe::clients::Plan;
use crate::providers::stripe::clients::Price;
use crate::providers::stripe::clients::Product;
use crate::providers::stripe::clients::ProductFeature;
use crate::providers::stripe::clients::PromotionCode;
use crate::providers::stripe::clients::Quote;
use crate::providers::stripe::clients::RadarEarlyFraudWarning;
use crate::providers::stripe::clients::RadarPaymentEvaluation;
use crate::providers::stripe::clients::RadarValueList;
use crate::providers::stripe::clients::RadarValueListItem;
use crate::providers::stripe::clients::Refund;
use crate::providers::stripe::clients::ReportingReportRun;
use crate::providers::stripe::clients::ReportingReportType;
use crate::providers::stripe::clients::Review;
use crate::providers::stripe::clients::ScheduledQueryRun;
use crate::providers::stripe::clients::SetupIntent;
use crate::providers::stripe::clients::ShippingRate;
use crate::providers::stripe::clients::SigmaSigmaApiQuery;
use crate::providers::stripe::clients::Source;
use crate::providers::stripe::clients::SourceMandateNotification;
use crate::providers::stripe::clients::SourceTransaction;
use crate::providers::stripe::clients::Subscription;
use crate::providers::stripe::clients::SubscriptionItem;
use crate::providers::stripe::clients::SubscriptionSchedule;
use crate::providers::stripe::clients::TaxAssociation;
use crate::providers::stripe::clients::TaxCalculation;
use crate::providers::stripe::clients::TaxCode;
use crate::providers::stripe::clients::TaxId;
use crate::providers::stripe::clients::TaxRate;
use crate::providers::stripe::clients::TaxRegistration;
use crate::providers::stripe::clients::TaxSettings;
use crate::providers::stripe::clients::TaxTransaction;
use crate::providers::stripe::clients::TerminalConfiguration;
use crate::providers::stripe::clients::TerminalConnectionToken;
use crate::providers::stripe::clients::TerminalLocation;
use crate::providers::stripe::clients::TerminalOnboardingLink;
use crate::providers::stripe::clients::TerminalReader;
use crate::providers::stripe::clients::TerminalRefund;
use crate::providers::stripe::clients::TestHelpersTestClock;
use crate::providers::stripe::clients::Token;
use crate::providers::stripe::clients::Topup;
use crate::providers::stripe::clients::Transfer;
use crate::providers::stripe::clients::TransferReversal;
use crate::providers::stripe::clients::TreasuryCreditReversal;
use crate::providers::stripe::clients::TreasuryDebitReversal;
use crate::providers::stripe::clients::TreasuryFinancialAccount;
use crate::providers::stripe::clients::TreasuryFinancialAccountFeatures;
use crate::providers::stripe::clients::TreasuryInboundTransfer;
use crate::providers::stripe::clients::TreasuryOutboundPayment;
use crate::providers::stripe::clients::TreasuryOutboundTransfer;
use crate::providers::stripe::clients::TreasuryReceivedCredit;
use crate::providers::stripe::clients::TreasuryReceivedDebit;
use crate::providers::stripe::clients::TreasuryTransaction;
use crate::providers::stripe::clients::TreasuryTransactionEntry;
use crate::providers::stripe::clients::WebhookEndpoint;
use crate::providers::stripe::clients::DeleteAccountsAccountArgs;
use crate::providers::stripe::clients::DeleteAccountsAccountBankAccountsIdArgs;
use crate::providers::stripe::clients::DeleteAccountsAccountExternalAccountsIdArgs;
use crate::providers::stripe::clients::DeleteAccountsAccountPeoplePersonArgs;
use crate::providers::stripe::clients::DeleteAccountsAccountPersonsPersonArgs;
use crate::providers::stripe::clients::DeleteApplePayDomainsDomainArgs;
use crate::providers::stripe::clients::DeleteCouponsCouponArgs;
use crate::providers::stripe::clients::DeleteCustomersCustomerArgs;
use crate::providers::stripe::clients::DeleteCustomersCustomerBankAccountsIdArgs;
use crate::providers::stripe::clients::DeleteCustomersCustomerCardsIdArgs;
use crate::providers::stripe::clients::DeleteCustomersCustomerDiscountArgs;
use crate::providers::stripe::clients::DeleteCustomersCustomerSourcesIdArgs;
use crate::providers::stripe::clients::DeleteCustomersCustomerSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::DeleteCustomersCustomerSubscriptionsSubscriptionExposedIdDiscountArgs;
use crate::providers::stripe::clients::DeleteCustomersCustomerTaxIdsIdArgs;
use crate::providers::stripe::clients::DeleteEphemeralKeysKeyArgs;
use crate::providers::stripe::clients::DeleteInvoiceitemsInvoiceitemArgs;
use crate::providers::stripe::clients::DeleteInvoicesInvoiceArgs;
use crate::providers::stripe::clients::DeletePlansPlanArgs;
use crate::providers::stripe::clients::DeleteProductsIdArgs;
use crate::providers::stripe::clients::DeleteProductsProductFeaturesIdArgs;
use crate::providers::stripe::clients::DeleteRadarValueListItemsItemArgs;
use crate::providers::stripe::clients::DeleteRadarValueListsValueListArgs;
use crate::providers::stripe::clients::DeleteSubscriptionItemsItemArgs;
use crate::providers::stripe::clients::DeleteSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::DeleteSubscriptionsSubscriptionExposedIdDiscountArgs;
use crate::providers::stripe::clients::DeleteTaxIdsIdArgs;
use crate::providers::stripe::clients::DeleteTerminalConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::DeleteTerminalLocationsLocationArgs;
use crate::providers::stripe::clients::DeleteTerminalReadersReaderArgs;
use crate::providers::stripe::clients::DeleteTestHelpersTestClocksTestClockArgs;
use crate::providers::stripe::clients::DeleteWebhookEndpointsWebhookEndpointArgs;
use crate::providers::stripe::clients::GetAccountArgs;
use crate::providers::stripe::clients::GetAccountsAccountArgs;
use crate::providers::stripe::clients::GetAccountsAccountBankAccountsIdArgs;
use crate::providers::stripe::clients::GetAccountsAccountCapabilitiesArgs;
use crate::providers::stripe::clients::GetAccountsAccountCapabilitiesCapabilityArgs;
use crate::providers::stripe::clients::GetAccountsAccountExternalAccountsArgs;
use crate::providers::stripe::clients::GetAccountsAccountExternalAccountsIdArgs;
use crate::providers::stripe::clients::GetAccountsAccountPeopleArgs;
use crate::providers::stripe::clients::GetAccountsAccountPeoplePersonArgs;
use crate::providers::stripe::clients::GetAccountsAccountPersonsArgs;
use crate::providers::stripe::clients::GetAccountsAccountPersonsPersonArgs;
use crate::providers::stripe::clients::GetAccountsArgs;
use crate::providers::stripe::clients::GetApplePayDomainsArgs;
use crate::providers::stripe::clients::GetApplePayDomainsDomainArgs;
use crate::providers::stripe::clients::GetApplicationFeesArgs;
use crate::providers::stripe::clients::GetApplicationFeesFeeRefundsIdArgs;
use crate::providers::stripe::clients::GetApplicationFeesIdArgs;
use crate::providers::stripe::clients::GetApplicationFeesIdRefundsArgs;
use crate::providers::stripe::clients::GetAppsSecretsArgs;
use crate::providers::stripe::clients::GetAppsSecretsFindArgs;
use crate::providers::stripe::clients::GetBalanceArgs;
use crate::providers::stripe::clients::GetBalanceHistoryArgs;
use crate::providers::stripe::clients::GetBalanceHistoryIdArgs;
use crate::providers::stripe::clients::GetBalanceSettingsArgs;
use crate::providers::stripe::clients::GetBalanceTransactionsArgs;
use crate::providers::stripe::clients::GetBalanceTransactionsIdArgs;
use crate::providers::stripe::clients::GetBillingAlertsArgs;
use crate::providers::stripe::clients::GetBillingAlertsIdArgs;
use crate::providers::stripe::clients::GetBillingCreditBalanceSummaryArgs;
use crate::providers::stripe::clients::GetBillingCreditBalanceTransactionsArgs;
use crate::providers::stripe::clients::GetBillingCreditBalanceTransactionsIdArgs;
use crate::providers::stripe::clients::GetBillingCreditGrantsArgs;
use crate::providers::stripe::clients::GetBillingCreditGrantsIdArgs;
use crate::providers::stripe::clients::GetBillingMetersArgs;
use crate::providers::stripe::clients::GetBillingMetersIdArgs;
use crate::providers::stripe::clients::GetBillingMetersIdEventSummariesArgs;
use crate::providers::stripe::clients::GetBillingPortalConfigurationsArgs;
use crate::providers::stripe::clients::GetBillingPortalConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::GetChargesArgs;
use crate::providers::stripe::clients::GetChargesChargeArgs;
use crate::providers::stripe::clients::GetChargesChargeDisputeArgs;
use crate::providers::stripe::clients::GetChargesChargeRefundsArgs;
use crate::providers::stripe::clients::GetChargesChargeRefundsRefundArgs;
use crate::providers::stripe::clients::GetChargesSearchArgs;
use crate::providers::stripe::clients::GetCheckoutSessionsArgs;
use crate::providers::stripe::clients::GetCheckoutSessionsSessionArgs;
use crate::providers::stripe::clients::GetCheckoutSessionsSessionLineItemsArgs;
use crate::providers::stripe::clients::GetClimateOrdersArgs;
use crate::providers::stripe::clients::GetClimateOrdersOrderArgs;
use crate::providers::stripe::clients::GetClimateProductsArgs;
use crate::providers::stripe::clients::GetClimateProductsProductArgs;
use crate::providers::stripe::clients::GetClimateSuppliersArgs;
use crate::providers::stripe::clients::GetClimateSuppliersSupplierArgs;
use crate::providers::stripe::clients::GetConfirmationTokensConfirmationTokenArgs;
use crate::providers::stripe::clients::GetCountrySpecsArgs;
use crate::providers::stripe::clients::GetCountrySpecsCountryArgs;
use crate::providers::stripe::clients::GetCouponsArgs;
use crate::providers::stripe::clients::GetCouponsCouponArgs;
use crate::providers::stripe::clients::GetCreditNotesArgs;
use crate::providers::stripe::clients::GetCreditNotesCreditNoteLinesArgs;
use crate::providers::stripe::clients::GetCreditNotesIdArgs;
use crate::providers::stripe::clients::GetCreditNotesPreviewArgs;
use crate::providers::stripe::clients::GetCreditNotesPreviewLinesArgs;
use crate::providers::stripe::clients::GetCustomersArgs;
use crate::providers::stripe::clients::GetCustomersCustomerArgs;
use crate::providers::stripe::clients::GetCustomersCustomerBalanceTransactionsArgs;
use crate::providers::stripe::clients::GetCustomersCustomerBalanceTransactionsTransactionArgs;
use crate::providers::stripe::clients::GetCustomersCustomerBankAccountsArgs;
use crate::providers::stripe::clients::GetCustomersCustomerBankAccountsIdArgs;
use crate::providers::stripe::clients::GetCustomersCustomerCardsArgs;
use crate::providers::stripe::clients::GetCustomersCustomerCardsIdArgs;
use crate::providers::stripe::clients::GetCustomersCustomerCashBalanceArgs;
use crate::providers::stripe::clients::GetCustomersCustomerCashBalanceTransactionsArgs;
use crate::providers::stripe::clients::GetCustomersCustomerCashBalanceTransactionsTransactionArgs;
use crate::providers::stripe::clients::GetCustomersCustomerDiscountArgs;
use crate::providers::stripe::clients::GetCustomersCustomerPaymentMethodsArgs;
use crate::providers::stripe::clients::GetCustomersCustomerPaymentMethodsPaymentMethodArgs;
use crate::providers::stripe::clients::GetCustomersCustomerSourcesArgs;
use crate::providers::stripe::clients::GetCustomersCustomerSourcesIdArgs;
use crate::providers::stripe::clients::GetCustomersCustomerSubscriptionsArgs;
use crate::providers::stripe::clients::GetCustomersCustomerSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::GetCustomersCustomerSubscriptionsSubscriptionExposedIdDiscountArgs;
use crate::providers::stripe::clients::GetCustomersCustomerTaxIdsArgs;
use crate::providers::stripe::clients::GetCustomersCustomerTaxIdsIdArgs;
use crate::providers::stripe::clients::GetCustomersSearchArgs;
use crate::providers::stripe::clients::GetDisputesArgs;
use crate::providers::stripe::clients::GetDisputesDisputeArgs;
use crate::providers::stripe::clients::GetEntitlementsActiveEntitlementsArgs;
use crate::providers::stripe::clients::GetEntitlementsActiveEntitlementsIdArgs;
use crate::providers::stripe::clients::GetEntitlementsFeaturesArgs;
use crate::providers::stripe::clients::GetEntitlementsFeaturesIdArgs;
use crate::providers::stripe::clients::GetEventsArgs;
use crate::providers::stripe::clients::GetEventsIdArgs;
use crate::providers::stripe::clients::GetExchangeRatesArgs;
use crate::providers::stripe::clients::GetExchangeRatesRateIdArgs;
use crate::providers::stripe::clients::GetFileLinksArgs;
use crate::providers::stripe::clients::GetFileLinksLinkArgs;
use crate::providers::stripe::clients::GetFilesArgs;
use crate::providers::stripe::clients::GetFilesFileArgs;
use crate::providers::stripe::clients::GetFinancialConnectionsAccountsAccountArgs;
use crate::providers::stripe::clients::GetFinancialConnectionsAccountsAccountOwnersArgs;
use crate::providers::stripe::clients::GetFinancialConnectionsAccountsArgs;
use crate::providers::stripe::clients::GetFinancialConnectionsSessionsSessionArgs;
use crate::providers::stripe::clients::GetFinancialConnectionsTransactionsArgs;
use crate::providers::stripe::clients::GetFinancialConnectionsTransactionsTransactionArgs;
use crate::providers::stripe::clients::GetForwardingRequestsArgs;
use crate::providers::stripe::clients::GetForwardingRequestsIdArgs;
use crate::providers::stripe::clients::GetIdentityVerificationReportsArgs;
use crate::providers::stripe::clients::GetIdentityVerificationReportsReportArgs;
use crate::providers::stripe::clients::GetIdentityVerificationSessionsArgs;
use crate::providers::stripe::clients::GetIdentityVerificationSessionsSessionArgs;
use crate::providers::stripe::clients::GetInvoicePaymentsArgs;
use crate::providers::stripe::clients::GetInvoicePaymentsInvoicePaymentArgs;
use crate::providers::stripe::clients::GetInvoiceRenderingTemplatesArgs;
use crate::providers::stripe::clients::GetInvoiceRenderingTemplatesTemplateArgs;
use crate::providers::stripe::clients::GetInvoiceitemsArgs;
use crate::providers::stripe::clients::GetInvoiceitemsInvoiceitemArgs;
use crate::providers::stripe::clients::GetInvoicesArgs;
use crate::providers::stripe::clients::GetInvoicesInvoiceArgs;
use crate::providers::stripe::clients::GetInvoicesInvoiceLinesArgs;
use crate::providers::stripe::clients::GetInvoicesSearchArgs;
use crate::providers::stripe::clients::GetIssuingAuthorizationsArgs;
use crate::providers::stripe::clients::GetIssuingAuthorizationsAuthorizationArgs;
use crate::providers::stripe::clients::GetIssuingCardholdersArgs;
use crate::providers::stripe::clients::GetIssuingCardholdersCardholderArgs;
use crate::providers::stripe::clients::GetIssuingCardsArgs;
use crate::providers::stripe::clients::GetIssuingCardsCardArgs;
use crate::providers::stripe::clients::GetIssuingDisputesArgs;
use crate::providers::stripe::clients::GetIssuingDisputesDisputeArgs;
use crate::providers::stripe::clients::GetIssuingPersonalizationDesignsArgs;
use crate::providers::stripe::clients::GetIssuingPersonalizationDesignsPersonalizationDesignArgs;
use crate::providers::stripe::clients::GetIssuingPhysicalBundlesArgs;
use crate::providers::stripe::clients::GetIssuingPhysicalBundlesPhysicalBundleArgs;
use crate::providers::stripe::clients::GetIssuingSettlementsSettlementArgs;
use crate::providers::stripe::clients::GetIssuingTokensArgs;
use crate::providers::stripe::clients::GetIssuingTokensTokenArgs;
use crate::providers::stripe::clients::GetIssuingTransactionsArgs;
use crate::providers::stripe::clients::GetIssuingTransactionsTransactionArgs;
use crate::providers::stripe::clients::GetLinkAccountSessionsSessionArgs;
use crate::providers::stripe::clients::GetLinkedAccountsAccountArgs;
use crate::providers::stripe::clients::GetLinkedAccountsAccountOwnersArgs;
use crate::providers::stripe::clients::GetLinkedAccountsArgs;
use crate::providers::stripe::clients::GetMandatesMandateArgs;
use crate::providers::stripe::clients::GetPaymentAttemptRecordsArgs;
use crate::providers::stripe::clients::GetPaymentAttemptRecordsIdArgs;
use crate::providers::stripe::clients::GetPaymentIntentsArgs;
use crate::providers::stripe::clients::GetPaymentIntentsIntentAmountDetailsLineItemsArgs;
use crate::providers::stripe::clients::GetPaymentIntentsIntentArgs;
use crate::providers::stripe::clients::GetPaymentIntentsSearchArgs;
use crate::providers::stripe::clients::GetPaymentLinksArgs;
use crate::providers::stripe::clients::GetPaymentLinksPaymentLinkArgs;
use crate::providers::stripe::clients::GetPaymentLinksPaymentLinkLineItemsArgs;
use crate::providers::stripe::clients::GetPaymentMethodConfigurationsArgs;
use crate::providers::stripe::clients::GetPaymentMethodConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::GetPaymentMethodDomainsArgs;
use crate::providers::stripe::clients::GetPaymentMethodDomainsPaymentMethodDomainArgs;
use crate::providers::stripe::clients::GetPaymentMethodsArgs;
use crate::providers::stripe::clients::GetPaymentMethodsPaymentMethodArgs;
use crate::providers::stripe::clients::GetPaymentRecordsIdArgs;
use crate::providers::stripe::clients::GetPayoutsArgs;
use crate::providers::stripe::clients::GetPayoutsPayoutArgs;
use crate::providers::stripe::clients::GetPlansArgs;
use crate::providers::stripe::clients::GetPlansPlanArgs;
use crate::providers::stripe::clients::GetPricesArgs;
use crate::providers::stripe::clients::GetPricesPriceArgs;
use crate::providers::stripe::clients::GetPricesSearchArgs;
use crate::providers::stripe::clients::GetProductsArgs;
use crate::providers::stripe::clients::GetProductsIdArgs;
use crate::providers::stripe::clients::GetProductsProductFeaturesArgs;
use crate::providers::stripe::clients::GetProductsProductFeaturesIdArgs;
use crate::providers::stripe::clients::GetProductsSearchArgs;
use crate::providers::stripe::clients::GetPromotionCodesArgs;
use crate::providers::stripe::clients::GetPromotionCodesPromotionCodeArgs;
use crate::providers::stripe::clients::GetQuotesArgs;
use crate::providers::stripe::clients::GetQuotesQuoteArgs;
use crate::providers::stripe::clients::GetQuotesQuoteComputedUpfrontLineItemsArgs;
use crate::providers::stripe::clients::GetQuotesQuoteLineItemsArgs;
use crate::providers::stripe::clients::GetQuotesQuotePdfArgs;
use crate::providers::stripe::clients::GetRadarEarlyFraudWarningsArgs;
use crate::providers::stripe::clients::GetRadarEarlyFraudWarningsEarlyFraudWarningArgs;
use crate::providers::stripe::clients::GetRadarValueListItemsArgs;
use crate::providers::stripe::clients::GetRadarValueListItemsItemArgs;
use crate::providers::stripe::clients::GetRadarValueListsArgs;
use crate::providers::stripe::clients::GetRadarValueListsValueListArgs;
use crate::providers::stripe::clients::GetRefundsArgs;
use crate::providers::stripe::clients::GetRefundsRefundArgs;
use crate::providers::stripe::clients::GetReportingReportRunsArgs;
use crate::providers::stripe::clients::GetReportingReportRunsReportRunArgs;
use crate::providers::stripe::clients::GetReportingReportTypesArgs;
use crate::providers::stripe::clients::GetReportingReportTypesReportTypeArgs;
use crate::providers::stripe::clients::GetReviewsArgs;
use crate::providers::stripe::clients::GetReviewsReviewArgs;
use crate::providers::stripe::clients::GetSetupAttemptsArgs;
use crate::providers::stripe::clients::GetSetupIntentsArgs;
use crate::providers::stripe::clients::GetSetupIntentsIntentArgs;
use crate::providers::stripe::clients::GetShippingRatesArgs;
use crate::providers::stripe::clients::GetShippingRatesShippingRateTokenArgs;
use crate::providers::stripe::clients::GetSigmaScheduledQueryRunsArgs;
use crate::providers::stripe::clients::GetSigmaScheduledQueryRunsScheduledQueryRunArgs;
use crate::providers::stripe::clients::GetSourcesSourceArgs;
use crate::providers::stripe::clients::GetSourcesSourceMandateNotificationsMandateNotificationArgs;
use crate::providers::stripe::clients::GetSourcesSourceSourceTransactionsArgs;
use crate::providers::stripe::clients::GetSourcesSourceSourceTransactionsSourceTransactionArgs;
use crate::providers::stripe::clients::GetSubscriptionItemsArgs;
use crate::providers::stripe::clients::GetSubscriptionItemsItemArgs;
use crate::providers::stripe::clients::GetSubscriptionSchedulesArgs;
use crate::providers::stripe::clients::GetSubscriptionSchedulesScheduleArgs;
use crate::providers::stripe::clients::GetSubscriptionsArgs;
use crate::providers::stripe::clients::GetSubscriptionsSearchArgs;
use crate::providers::stripe::clients::GetSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::GetTaxAssociationsFindArgs;
use crate::providers::stripe::clients::GetTaxCalculationsCalculationArgs;
use crate::providers::stripe::clients::GetTaxCalculationsCalculationLineItemsArgs;
use crate::providers::stripe::clients::GetTaxCodesArgs;
use crate::providers::stripe::clients::GetTaxCodesIdArgs;
use crate::providers::stripe::clients::GetTaxIdsArgs;
use crate::providers::stripe::clients::GetTaxIdsIdArgs;
use crate::providers::stripe::clients::GetTaxRatesArgs;
use crate::providers::stripe::clients::GetTaxRatesTaxRateArgs;
use crate::providers::stripe::clients::GetTaxRegistrationsArgs;
use crate::providers::stripe::clients::GetTaxRegistrationsIdArgs;
use crate::providers::stripe::clients::GetTaxSettingsArgs;
use crate::providers::stripe::clients::GetTaxTransactionsTransactionArgs;
use crate::providers::stripe::clients::GetTaxTransactionsTransactionLineItemsArgs;
use crate::providers::stripe::clients::GetTerminalConfigurationsArgs;
use crate::providers::stripe::clients::GetTerminalConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::GetTerminalLocationsArgs;
use crate::providers::stripe::clients::GetTerminalLocationsLocationArgs;
use crate::providers::stripe::clients::GetTerminalReadersArgs;
use crate::providers::stripe::clients::GetTerminalReadersReaderArgs;
use crate::providers::stripe::clients::GetTestHelpersTestClocksArgs;
use crate::providers::stripe::clients::GetTestHelpersTestClocksTestClockArgs;
use crate::providers::stripe::clients::GetTokensTokenArgs;
use crate::providers::stripe::clients::GetTopupsArgs;
use crate::providers::stripe::clients::GetTopupsTopupArgs;
use crate::providers::stripe::clients::GetTransfersArgs;
use crate::providers::stripe::clients::GetTransfersIdReversalsArgs;
use crate::providers::stripe::clients::GetTransfersTransferArgs;
use crate::providers::stripe::clients::GetTransfersTransferReversalsIdArgs;
use crate::providers::stripe::clients::GetTreasuryCreditReversalsArgs;
use crate::providers::stripe::clients::GetTreasuryCreditReversalsCreditReversalArgs;
use crate::providers::stripe::clients::GetTreasuryDebitReversalsArgs;
use crate::providers::stripe::clients::GetTreasuryDebitReversalsDebitReversalArgs;
use crate::providers::stripe::clients::GetTreasuryFinancialAccountsArgs;
use crate::providers::stripe::clients::GetTreasuryFinancialAccountsFinancialAccountArgs;
use crate::providers::stripe::clients::GetTreasuryFinancialAccountsFinancialAccountFeaturesArgs;
use crate::providers::stripe::clients::GetTreasuryInboundTransfersArgs;
use crate::providers::stripe::clients::GetTreasuryInboundTransfersIdArgs;
use crate::providers::stripe::clients::GetTreasuryOutboundPaymentsArgs;
use crate::providers::stripe::clients::GetTreasuryOutboundPaymentsIdArgs;
use crate::providers::stripe::clients::GetTreasuryOutboundTransfersArgs;
use crate::providers::stripe::clients::GetTreasuryOutboundTransfersOutboundTransferArgs;
use crate::providers::stripe::clients::GetTreasuryReceivedCreditsArgs;
use crate::providers::stripe::clients::GetTreasuryReceivedCreditsIdArgs;
use crate::providers::stripe::clients::GetTreasuryReceivedDebitsArgs;
use crate::providers::stripe::clients::GetTreasuryReceivedDebitsIdArgs;
use crate::providers::stripe::clients::GetTreasuryTransactionEntriesArgs;
use crate::providers::stripe::clients::GetTreasuryTransactionEntriesIdArgs;
use crate::providers::stripe::clients::GetTreasuryTransactionsArgs;
use crate::providers::stripe::clients::GetTreasuryTransactionsIdArgs;
use crate::providers::stripe::clients::GetWebhookEndpointsArgs;
use crate::providers::stripe::clients::GetWebhookEndpointsWebhookEndpointArgs;
use crate::providers::stripe::clients::PostAccountsAccountArgs;
use crate::providers::stripe::clients::PostAccountsAccountBankAccountsArgs;
use crate::providers::stripe::clients::PostAccountsAccountBankAccountsIdArgs;
use crate::providers::stripe::clients::PostAccountsAccountCapabilitiesCapabilityArgs;
use crate::providers::stripe::clients::PostAccountsAccountExternalAccountsArgs;
use crate::providers::stripe::clients::PostAccountsAccountExternalAccountsIdArgs;
use crate::providers::stripe::clients::PostAccountsAccountLoginLinksArgs;
use crate::providers::stripe::clients::PostAccountsAccountPeopleArgs;
use crate::providers::stripe::clients::PostAccountsAccountPeoplePersonArgs;
use crate::providers::stripe::clients::PostAccountsAccountPersonsArgs;
use crate::providers::stripe::clients::PostAccountsAccountPersonsPersonArgs;
use crate::providers::stripe::clients::PostAccountsAccountRejectArgs;
use crate::providers::stripe::clients::PostApplicationFeesFeeRefundsIdArgs;
use crate::providers::stripe::clients::PostApplicationFeesIdRefundArgs;
use crate::providers::stripe::clients::PostApplicationFeesIdRefundsArgs;
use crate::providers::stripe::clients::PostBillingAlertsIdActivateArgs;
use crate::providers::stripe::clients::PostBillingAlertsIdArchiveArgs;
use crate::providers::stripe::clients::PostBillingAlertsIdDeactivateArgs;
use crate::providers::stripe::clients::PostBillingCreditGrantsIdArgs;
use crate::providers::stripe::clients::PostBillingCreditGrantsIdExpireArgs;
use crate::providers::stripe::clients::PostBillingCreditGrantsIdVoidArgs;
use crate::providers::stripe::clients::PostBillingMetersIdArgs;
use crate::providers::stripe::clients::PostBillingMetersIdDeactivateArgs;
use crate::providers::stripe::clients::PostBillingMetersIdReactivateArgs;
use crate::providers::stripe::clients::PostBillingPortalConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::PostChargesChargeArgs;
use crate::providers::stripe::clients::PostChargesChargeCaptureArgs;
use crate::providers::stripe::clients::PostChargesChargeDisputeArgs;
use crate::providers::stripe::clients::PostChargesChargeDisputeCloseArgs;
use crate::providers::stripe::clients::PostChargesChargeRefundArgs;
use crate::providers::stripe::clients::PostChargesChargeRefundsArgs;
use crate::providers::stripe::clients::PostChargesChargeRefundsRefundArgs;
use crate::providers::stripe::clients::PostCheckoutSessionsSessionArgs;
use crate::providers::stripe::clients::PostCheckoutSessionsSessionExpireArgs;
use crate::providers::stripe::clients::PostClimateOrdersOrderArgs;
use crate::providers::stripe::clients::PostClimateOrdersOrderCancelArgs;
use crate::providers::stripe::clients::PostCouponsCouponArgs;
use crate::providers::stripe::clients::PostCreditNotesIdArgs;
use crate::providers::stripe::clients::PostCreditNotesIdVoidArgs;
use crate::providers::stripe::clients::PostCustomersCustomerArgs;
use crate::providers::stripe::clients::PostCustomersCustomerBalanceTransactionsArgs;
use crate::providers::stripe::clients::PostCustomersCustomerBalanceTransactionsTransactionArgs;
use crate::providers::stripe::clients::PostCustomersCustomerBankAccountsArgs;
use crate::providers::stripe::clients::PostCustomersCustomerBankAccountsIdArgs;
use crate::providers::stripe::clients::PostCustomersCustomerBankAccountsIdVerifyArgs;
use crate::providers::stripe::clients::PostCustomersCustomerCardsArgs;
use crate::providers::stripe::clients::PostCustomersCustomerCardsIdArgs;
use crate::providers::stripe::clients::PostCustomersCustomerCashBalanceArgs;
use crate::providers::stripe::clients::PostCustomersCustomerFundingInstructionsArgs;
use crate::providers::stripe::clients::PostCustomersCustomerSourcesArgs;
use crate::providers::stripe::clients::PostCustomersCustomerSourcesIdArgs;
use crate::providers::stripe::clients::PostCustomersCustomerSourcesIdVerifyArgs;
use crate::providers::stripe::clients::PostCustomersCustomerSubscriptionsArgs;
use crate::providers::stripe::clients::PostCustomersCustomerSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::PostCustomersCustomerTaxIdsArgs;
use crate::providers::stripe::clients::PostDisputesDisputeArgs;
use crate::providers::stripe::clients::PostDisputesDisputeCloseArgs;
use crate::providers::stripe::clients::PostEntitlementsFeaturesIdArgs;
use crate::providers::stripe::clients::PostExternalAccountsIdArgs;
use crate::providers::stripe::clients::PostFileLinksLinkArgs;
use crate::providers::stripe::clients::PostFinancialConnectionsAccountsAccountDisconnectArgs;
use crate::providers::stripe::clients::PostFinancialConnectionsAccountsAccountRefreshArgs;
use crate::providers::stripe::clients::PostFinancialConnectionsAccountsAccountSubscribeArgs;
use crate::providers::stripe::clients::PostFinancialConnectionsAccountsAccountUnsubscribeArgs;
use crate::providers::stripe::clients::PostIdentityVerificationSessionsSessionArgs;
use crate::providers::stripe::clients::PostIdentityVerificationSessionsSessionCancelArgs;
use crate::providers::stripe::clients::PostIdentityVerificationSessionsSessionRedactArgs;
use crate::providers::stripe::clients::PostInvoiceRenderingTemplatesTemplateArchiveArgs;
use crate::providers::stripe::clients::PostInvoiceRenderingTemplatesTemplateUnarchiveArgs;
use crate::providers::stripe::clients::PostInvoiceitemsInvoiceitemArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceAddLinesArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceAttachPaymentArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceFinalizeArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceLinesLineItemIdArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceMarkUncollectibleArgs;
use crate::providers::stripe::clients::PostInvoicesInvoicePayArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceRemoveLinesArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceSendArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceUpdateLinesArgs;
use crate::providers::stripe::clients::PostInvoicesInvoiceVoidArgs;
use crate::providers::stripe::clients::PostIssuingAuthorizationsAuthorizationApproveArgs;
use crate::providers::stripe::clients::PostIssuingAuthorizationsAuthorizationArgs;
use crate::providers::stripe::clients::PostIssuingAuthorizationsAuthorizationDeclineArgs;
use crate::providers::stripe::clients::PostIssuingCardholdersCardholderArgs;
use crate::providers::stripe::clients::PostIssuingCardsCardArgs;
use crate::providers::stripe::clients::PostIssuingDisputesDisputeArgs;
use crate::providers::stripe::clients::PostIssuingDisputesDisputeSubmitArgs;
use crate::providers::stripe::clients::PostIssuingPersonalizationDesignsPersonalizationDesignArgs;
use crate::providers::stripe::clients::PostIssuingSettlementsSettlementArgs;
use crate::providers::stripe::clients::PostIssuingTokensTokenArgs;
use crate::providers::stripe::clients::PostIssuingTransactionsTransactionArgs;
use crate::providers::stripe::clients::PostLinkedAccountsAccountDisconnectArgs;
use crate::providers::stripe::clients::PostLinkedAccountsAccountRefreshArgs;
use crate::providers::stripe::clients::PostPaymentIntentsIntentApplyCustomerBalanceArgs;
use crate::providers::stripe::clients::PostPaymentIntentsIntentArgs;
use crate::providers::stripe::clients::PostPaymentIntentsIntentCancelArgs;
use crate::providers::stripe::clients::PostPaymentIntentsIntentCaptureArgs;
use crate::providers::stripe::clients::PostPaymentIntentsIntentConfirmArgs;
use crate::providers::stripe::clients::PostPaymentIntentsIntentIncrementAuthorizationArgs;
use crate::providers::stripe::clients::PostPaymentIntentsIntentVerifyMicrodepositsArgs;
use crate::providers::stripe::clients::PostPaymentLinksPaymentLinkArgs;
use crate::providers::stripe::clients::PostPaymentMethodConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::PostPaymentMethodDomainsPaymentMethodDomainArgs;
use crate::providers::stripe::clients::PostPaymentMethodDomainsPaymentMethodDomainValidateArgs;
use crate::providers::stripe::clients::PostPaymentMethodsPaymentMethodArgs;
use crate::providers::stripe::clients::PostPaymentMethodsPaymentMethodAttachArgs;
use crate::providers::stripe::clients::PostPaymentMethodsPaymentMethodDetachArgs;
use crate::providers::stripe::clients::PostPaymentRecordsIdReportPaymentAttemptArgs;
use crate::providers::stripe::clients::PostPaymentRecordsIdReportPaymentAttemptCanceledArgs;
use crate::providers::stripe::clients::PostPaymentRecordsIdReportPaymentAttemptFailedArgs;
use crate::providers::stripe::clients::PostPaymentRecordsIdReportPaymentAttemptGuaranteedArgs;
use crate::providers::stripe::clients::PostPaymentRecordsIdReportPaymentAttemptInformationalArgs;
use crate::providers::stripe::clients::PostPaymentRecordsIdReportRefundArgs;
use crate::providers::stripe::clients::PostPayoutsPayoutArgs;
use crate::providers::stripe::clients::PostPayoutsPayoutCancelArgs;
use crate::providers::stripe::clients::PostPayoutsPayoutReverseArgs;
use crate::providers::stripe::clients::PostPlansPlanArgs;
use crate::providers::stripe::clients::PostPricesPriceArgs;
use crate::providers::stripe::clients::PostProductsIdArgs;
use crate::providers::stripe::clients::PostProductsProductFeaturesArgs;
use crate::providers::stripe::clients::PostPromotionCodesPromotionCodeArgs;
use crate::providers::stripe::clients::PostQuotesQuoteAcceptArgs;
use crate::providers::stripe::clients::PostQuotesQuoteArgs;
use crate::providers::stripe::clients::PostQuotesQuoteCancelArgs;
use crate::providers::stripe::clients::PostQuotesQuoteFinalizeArgs;
use crate::providers::stripe::clients::PostRadarValueListsValueListArgs;
use crate::providers::stripe::clients::PostRefundsRefundArgs;
use crate::providers::stripe::clients::PostRefundsRefundCancelArgs;
use crate::providers::stripe::clients::PostReviewsReviewApproveArgs;
use crate::providers::stripe::clients::PostSetupIntentsIntentArgs;
use crate::providers::stripe::clients::PostSetupIntentsIntentCancelArgs;
use crate::providers::stripe::clients::PostSetupIntentsIntentConfirmArgs;
use crate::providers::stripe::clients::PostSetupIntentsIntentVerifyMicrodepositsArgs;
use crate::providers::stripe::clients::PostShippingRatesShippingRateTokenArgs;
use crate::providers::stripe::clients::PostSigmaSavedQueriesIdArgs;
use crate::providers::stripe::clients::PostSourcesSourceArgs;
use crate::providers::stripe::clients::PostSourcesSourceVerifyArgs;
use crate::providers::stripe::clients::PostSubscriptionItemsItemArgs;
use crate::providers::stripe::clients::PostSubscriptionSchedulesScheduleArgs;
use crate::providers::stripe::clients::PostSubscriptionSchedulesScheduleCancelArgs;
use crate::providers::stripe::clients::PostSubscriptionSchedulesScheduleReleaseArgs;
use crate::providers::stripe::clients::PostSubscriptionsSubscriptionExposedIdArgs;
use crate::providers::stripe::clients::PostSubscriptionsSubscriptionMigrateArgs;
use crate::providers::stripe::clients::PostSubscriptionsSubscriptionResumeArgs;
use crate::providers::stripe::clients::PostTaxRatesTaxRateArgs;
use crate::providers::stripe::clients::PostTaxRegistrationsIdArgs;
use crate::providers::stripe::clients::PostTerminalConfigurationsConfigurationArgs;
use crate::providers::stripe::clients::PostTerminalLocationsLocationArgs;
use crate::providers::stripe::clients::PostTerminalReadersReaderArgs;
use crate::providers::stripe::clients::PostTerminalReadersReaderCancelActionArgs;
use crate::providers::stripe::clients::PostTerminalReadersReaderCollectInputsArgs;
use crate::providers::stripe::clients::PostTerminalReadersReaderCollectPaymentMethodArgs;
use crate::providers::stripe::clients::PostTerminalReadersReaderConfirmPaymentIntentArgs;
use crate::providers::stripe::clients::PostTerminalReadersReaderProcessPaymentIntentArgs;
use crate::providers::stripe::clients::PostTerminalReadersReaderProcessSetupIntentArgs;
use crate::providers::stripe::clients::PostTerminalReadersReaderRefundPaymentArgs;
use crate::providers::stripe::clients::PostTerminalReadersReaderSetReaderDisplayArgs;
use crate::providers::stripe::clients::PostTestHelpersCustomersCustomerFundCashBalanceArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingAuthorizationsAuthorizationCaptureArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingAuthorizationsAuthorizationExpireArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingAuthorizationsAuthorizationFinalizeAmountArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingAuthorizationsAuthorizationFraudChallengesRespondArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingAuthorizationsAuthorizationIncrementArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingAuthorizationsAuthorizationReverseArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingCardsCardShippingDeliverArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingCardsCardShippingFailArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingCardsCardShippingReturnArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingCardsCardShippingShipArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingCardsCardShippingSubmitArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingPersonalizationDesignsPersonalizationDesignActivateArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingPersonalizationDesignsPersonalizationDesignDeactivateArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingPersonalizationDesignsPersonalizationDesignRejectArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingSettlementsSettlementCompleteArgs;
use crate::providers::stripe::clients::PostTestHelpersIssuingTransactionsTransactionRefundArgs;
use crate::providers::stripe::clients::PostTestHelpersRefundsRefundExpireArgs;
use crate::providers::stripe::clients::PostTestHelpersTerminalReadersReaderPresentPaymentMethodArgs;
use crate::providers::stripe::clients::PostTestHelpersTerminalReadersReaderSucceedInputCollectionArgs;
use crate::providers::stripe::clients::PostTestHelpersTerminalReadersReaderTimeoutInputCollectionArgs;
use crate::providers::stripe::clients::PostTestHelpersTestClocksTestClockAdvanceArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryInboundTransfersIdFailArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryInboundTransfersIdReturnArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryInboundTransfersIdSucceedArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryOutboundPaymentsIdArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryOutboundPaymentsIdFailArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryOutboundPaymentsIdPostArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryOutboundPaymentsIdReturnArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryOutboundTransfersOutboundTransferArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryOutboundTransfersOutboundTransferFailArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryOutboundTransfersOutboundTransferPostArgs;
use crate::providers::stripe::clients::PostTestHelpersTreasuryOutboundTransfersOutboundTransferReturnArgs;
use crate::providers::stripe::clients::PostTopupsTopupArgs;
use crate::providers::stripe::clients::PostTopupsTopupCancelArgs;
use crate::providers::stripe::clients::PostTransfersIdReversalsArgs;
use crate::providers::stripe::clients::PostTransfersTransferArgs;
use crate::providers::stripe::clients::PostTransfersTransferReversalsIdArgs;
use crate::providers::stripe::clients::PostTreasuryFinancialAccountsFinancialAccountArgs;
use crate::providers::stripe::clients::PostTreasuryFinancialAccountsFinancialAccountCloseArgs;
use crate::providers::stripe::clients::PostTreasuryFinancialAccountsFinancialAccountFeaturesArgs;
use crate::providers::stripe::clients::PostTreasuryInboundTransfersInboundTransferCancelArgs;
use crate::providers::stripe::clients::PostTreasuryOutboundPaymentsIdCancelArgs;
use crate::providers::stripe::clients::PostTreasuryOutboundTransfersOutboundTransferCancelArgs;
use crate::providers::stripe::clients::PostWebhookEndpointsWebhookEndpointArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// StripeProvider with automatic state tracking.
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
/// let provider = StripeProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct StripeProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> StripeProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new StripeProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new StripeProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Get account.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_account(
        &self,
        args: &GetAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_account_builder(
            &self.http_client,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get accounts.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts(
        &self,
        args: &GetAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get accounts account.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account(
        &self,
        args: &GetAccountsAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_builder(
            &self.http_client,
            &args.account,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the ExternalAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_bank_accounts(
        &self,
        args: &PostAccountsAccountBankAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalAccount, ProviderError<ApiError>>,
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

    /// Get accounts account bank accounts id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExternalAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account_bank_accounts_id(
        &self,
        args: &GetAccountsAccountBankAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_bank_accounts_id_builder(
            &self.http_client,
            &args.account,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_bank_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the ExternalAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_bank_accounts_id(
        &self,
        args: &PostAccountsAccountBankAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalAccount, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the DeletedExternalAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_accounts_account_bank_accounts_id(
        &self,
        args: &DeleteAccountsAccountBankAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedExternalAccount, ProviderError<ApiError>>,
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

    /// Get accounts account capabilities.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account_capabilities(
        &self,
        args: &GetAccountsAccountCapabilitiesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_capabilities_builder(
            &self.http_client,
            &args.account,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_capabilities_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get accounts account capabilities capability.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account_capabilities_capability(
        &self,
        args: &GetAccountsAccountCapabilitiesCapabilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Capability, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_capabilities_capability_builder(
            &self.http_client,
            &args.account,
            &args.capability,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_capabilities_capability_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get accounts account external accounts.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account_external_accounts(
        &self,
        args: &GetAccountsAccountExternalAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_external_accounts_builder(
            &self.http_client,
            &args.account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.object,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_external_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the ExternalAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_external_accounts(
        &self,
        args: &PostAccountsAccountExternalAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalAccount, ProviderError<ApiError>>,
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

    /// Get accounts account external accounts id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExternalAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account_external_accounts_id(
        &self,
        args: &GetAccountsAccountExternalAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_external_accounts_id_builder(
            &self.http_client,
            &args.account,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_external_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the ExternalAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_accounts_account_external_accounts_id(
        &self,
        args: &PostAccountsAccountExternalAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalAccount, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the DeletedExternalAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_accounts_account_external_accounts_id(
        &self,
        args: &DeleteAccountsAccountExternalAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeletedExternalAccount, ProviderError<ApiError>>,
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

    /// Get accounts account people.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account_people(
        &self,
        args: &GetAccountsAccountPeopleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_people_builder(
            &self.http_client,
            &args.account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.relationship,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_people_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get accounts account people person.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account_people_person(
        &self,
        args: &GetAccountsAccountPeoplePersonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_people_person_builder(
            &self.http_client,
            &args.account,
            &args.person,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_people_person_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get accounts account persons.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account_persons(
        &self,
        args: &GetAccountsAccountPersonsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_persons_builder(
            &self.http_client,
            &args.account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.relationship,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_persons_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get accounts account persons person.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_accounts_account_persons_person(
        &self,
        args: &GetAccountsAccountPersonsPersonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_accounts_account_persons_person_builder(
            &self.http_client,
            &args.account,
            &args.person,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_accounts_account_persons_person_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get apple pay domains.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_apple_pay_domains(
        &self,
        args: &GetApplePayDomainsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_apple_pay_domains_builder(
            &self.http_client,
            &args.domain_name,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_apple_pay_domains_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get apple pay domains domain.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_apple_pay_domains_domain(
        &self,
        args: &GetApplePayDomainsDomainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApplePayDomain, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_apple_pay_domains_domain_builder(
            &self.http_client,
            &args.domain,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_apple_pay_domains_domain_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get application fees.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_application_fees(
        &self,
        args: &GetApplicationFeesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_application_fees_builder(
            &self.http_client,
            &args.charge,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_application_fees_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get application fees fee refunds id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_application_fees_fee_refunds_id(
        &self,
        args: &GetApplicationFeesFeeRefundsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FeeRefund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_application_fees_fee_refunds_id_builder(
            &self.http_client,
            &args.fee,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_application_fees_fee_refunds_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get application fees id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_application_fees_id(
        &self,
        args: &GetApplicationFeesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApplicationFee, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_application_fees_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_application_fees_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get application fees id refunds.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_application_fees_id_refunds(
        &self,
        args: &GetApplicationFeesIdRefundsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_application_fees_id_refunds_builder(
            &self.http_client,
            &args.id,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_application_fees_id_refunds_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get apps secrets.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_apps_secrets(
        &self,
        args: &GetAppsSecretsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_apps_secrets_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.scope,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_apps_secrets_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get apps secrets find.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_apps_secrets_find(
        &self,
        args: &GetAppsSecretsFindArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppsSecret, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_apps_secrets_find_builder(
            &self.http_client,
            &args.expand,
            &args.name,
            &args.scope,
        )
        .map_err(ProviderError::Api)?;

        let task = get_apps_secrets_find_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get balance.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Balance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_balance(
        &self,
        args: &GetBalanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Balance, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_balance_builder(
            &self.http_client,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_balance_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get balance history.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_balance_history(
        &self,
        args: &GetBalanceHistoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_balance_history_builder(
            &self.http_client,
            &args.created,
            &args.currency,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.payout,
            &args.source,
            &args.starting_after,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = get_balance_history_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get balance history id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BalanceTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_balance_history_id(
        &self,
        args: &GetBalanceHistoryIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BalanceTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_balance_history_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_balance_history_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get balance settings.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_balance_settings(
        &self,
        args: &GetBalanceSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BalanceSettings, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_balance_settings_builder(
            &self.http_client,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_balance_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get balance transactions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_balance_transactions(
        &self,
        args: &GetBalanceTransactionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_balance_transactions_builder(
            &self.http_client,
            &args.created,
            &args.currency,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.payout,
            &args.source,
            &args.starting_after,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = get_balance_transactions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get balance transactions id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BalanceTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_balance_transactions_id(
        &self,
        args: &GetBalanceTransactionsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BalanceTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_balance_transactions_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_balance_transactions_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get billing alerts.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_alerts(
        &self,
        args: &GetBillingAlertsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_alerts_builder(
            &self.http_client,
            &args.alert_type,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.meter,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_alerts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get billing alerts id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_alerts_id(
        &self,
        args: &GetBillingAlertsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAlert, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_alerts_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_alerts_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get billing credit balance summary.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingCreditBalanceSummary result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_credit_balance_summary(
        &self,
        args: &GetBillingCreditBalanceSummaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingCreditBalanceSummary, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_credit_balance_summary_builder(
            &self.http_client,
            &args.customer,
            &args.customer_account,
            &args.expand,
            &args.filter,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_credit_balance_summary_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get billing credit balance transactions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_credit_balance_transactions(
        &self,
        args: &GetBillingCreditBalanceTransactionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_credit_balance_transactions_builder(
            &self.http_client,
            &args.credit_grant,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_credit_balance_transactions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get billing credit balance transactions id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingCreditBalanceTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_credit_balance_transactions_id(
        &self,
        args: &GetBillingCreditBalanceTransactionsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingCreditBalanceTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_credit_balance_transactions_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_credit_balance_transactions_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get billing credit grants.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_credit_grants(
        &self,
        args: &GetBillingCreditGrantsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_credit_grants_builder(
            &self.http_client,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_credit_grants_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get billing credit grants id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_credit_grants_id(
        &self,
        args: &GetBillingCreditGrantsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingCreditGrant, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_credit_grants_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_credit_grants_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get billing meters.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_meters(
        &self,
        args: &GetBillingMetersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_meters_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_meters_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get billing meters id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_meters_id(
        &self,
        args: &GetBillingMetersIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingMeter, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_meters_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_meters_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get billing meters id event summaries.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_meters_id_event_summaries(
        &self,
        args: &GetBillingMetersIdEventSummariesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_meters_id_event_summaries_builder(
            &self.http_client,
            &args.id,
            &args.customer,
            &args.end_time,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.start_time,
            &args.starting_after,
            &args.value_grouping_window,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_meters_id_event_summaries_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get billing portal configurations.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_portal_configurations(
        &self,
        args: &GetBillingPortalConfigurationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_portal_configurations_builder(
            &self.http_client,
            &args.active,
            &args.ending_before,
            &args.expand,
            &args.is_default,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_portal_configurations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get billing portal configurations configuration.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_billing_portal_configurations_configuration(
        &self,
        args: &GetBillingPortalConfigurationsConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingPortalConfiguration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_billing_portal_configurations_configuration_builder(
            &self.http_client,
            &args.configuration,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_billing_portal_configurations_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get charges.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_charges(
        &self,
        args: &GetChargesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_charges_builder(
            &self.http_client,
            &args.created,
            &args.customer,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.payment_intent,
            &args.starting_after,
            &args.transfer_group,
        )
        .map_err(ProviderError::Api)?;

        let task = get_charges_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get charges search.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_charges_search(
        &self,
        args: &GetChargesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_charges_search_builder(
            &self.http_client,
            &args.expand,
            &args.limit,
            &args.page,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = get_charges_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get charges charge.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_charges_charge(
        &self,
        args: &GetChargesChargeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Charge, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_charges_charge_builder(
            &self.http_client,
            &args.charge,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_charges_charge_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get charges charge dispute.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_charges_charge_dispute(
        &self,
        args: &GetChargesChargeDisputeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_charges_charge_dispute_builder(
            &self.http_client,
            &args.charge,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_charges_charge_dispute_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get charges charge refunds.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_charges_charge_refunds(
        &self,
        args: &GetChargesChargeRefundsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_charges_charge_refunds_builder(
            &self.http_client,
            &args.charge,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_charges_charge_refunds_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get charges charge refunds refund.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_charges_charge_refunds_refund(
        &self,
        args: &GetChargesChargeRefundsRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Refund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_charges_charge_refunds_refund_builder(
            &self.http_client,
            &args.charge,
            &args.refund,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_charges_charge_refunds_refund_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get checkout sessions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_checkout_sessions(
        &self,
        args: &GetCheckoutSessionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_checkout_sessions_builder(
            &self.http_client,
            &args.created,
            &args.customer,
            &args.customer_account,
            &args.customer_details,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.payment_intent,
            &args.payment_link,
            &args.starting_after,
            &args.status,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = get_checkout_sessions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get checkout sessions session.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_checkout_sessions_session(
        &self,
        args: &GetCheckoutSessionsSessionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckoutSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_checkout_sessions_session_builder(
            &self.http_client,
            &args.session,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_checkout_sessions_session_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get checkout sessions session line items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_checkout_sessions_session_line_items(
        &self,
        args: &GetCheckoutSessionsSessionLineItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_checkout_sessions_session_line_items_builder(
            &self.http_client,
            &args.session,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_checkout_sessions_session_line_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get climate orders.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_climate_orders(
        &self,
        args: &GetClimateOrdersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_climate_orders_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_climate_orders_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get climate orders order.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_climate_orders_order(
        &self,
        args: &GetClimateOrdersOrderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClimateOrder, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_climate_orders_order_builder(
            &self.http_client,
            &args.order,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_climate_orders_order_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get climate products.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_climate_products(
        &self,
        args: &GetClimateProductsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_climate_products_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_climate_products_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get climate products product.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClimateProduct result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_climate_products_product(
        &self,
        args: &GetClimateProductsProductArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClimateProduct, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_climate_products_product_builder(
            &self.http_client,
            &args.product,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_climate_products_product_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get climate suppliers.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_climate_suppliers(
        &self,
        args: &GetClimateSuppliersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_climate_suppliers_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_climate_suppliers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get climate suppliers supplier.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClimateSupplier result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_climate_suppliers_supplier(
        &self,
        args: &GetClimateSuppliersSupplierArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClimateSupplier, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_climate_suppliers_supplier_builder(
            &self.http_client,
            &args.supplier,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_climate_suppliers_supplier_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get confirmation tokens confirmation token.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_confirmation_tokens_confirmation_token(
        &self,
        args: &GetConfirmationTokensConfirmationTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConfirmationToken, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_confirmation_tokens_confirmation_token_builder(
            &self.http_client,
            &args.confirmation_token,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_confirmation_tokens_confirmation_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get country specs.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_country_specs(
        &self,
        args: &GetCountrySpecsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_country_specs_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_country_specs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get country specs country.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CountrySpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_country_specs_country(
        &self,
        args: &GetCountrySpecsCountryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CountrySpec, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_country_specs_country_builder(
            &self.http_client,
            &args.country,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_country_specs_country_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get coupons.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_coupons(
        &self,
        args: &GetCouponsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_coupons_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_coupons_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get coupons coupon.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_coupons_coupon(
        &self,
        args: &GetCouponsCouponArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Coupon, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_coupons_coupon_builder(
            &self.http_client,
            &args.coupon,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_coupons_coupon_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get credit notes.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_credit_notes(
        &self,
        args: &GetCreditNotesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_credit_notes_builder(
            &self.http_client,
            &args.created,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.invoice,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_credit_notes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get credit notes preview.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_credit_notes_preview(
        &self,
        args: &GetCreditNotesPreviewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreditNote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_credit_notes_preview_builder(
            &self.http_client,
            &args.amount,
            &args.credit_amount,
            &args.effective_at,
            &args.email_type,
            &args.expand,
            &args.invoice,
            &args.lines,
            &args.memo,
            &args.metadata,
            &args.out_of_band_amount,
            &args.reason,
            &args.refund_amount,
            &args.refunds,
            &args.shipping_cost,
        )
        .map_err(ProviderError::Api)?;

        let task = get_credit_notes_preview_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get credit notes preview lines.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_credit_notes_preview_lines(
        &self,
        args: &GetCreditNotesPreviewLinesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_credit_notes_preview_lines_builder(
            &self.http_client,
            &args.amount,
            &args.credit_amount,
            &args.effective_at,
            &args.email_type,
            &args.ending_before,
            &args.expand,
            &args.invoice,
            &args.limit,
            &args.lines,
            &args.memo,
            &args.metadata,
            &args.out_of_band_amount,
            &args.reason,
            &args.refund_amount,
            &args.refunds,
            &args.shipping_cost,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_credit_notes_preview_lines_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get credit notes credit note lines.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_credit_notes_credit_note_lines(
        &self,
        args: &GetCreditNotesCreditNoteLinesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_credit_notes_credit_note_lines_builder(
            &self.http_client,
            &args.credit_note,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_credit_notes_credit_note_lines_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get credit notes id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_credit_notes_id(
        &self,
        args: &GetCreditNotesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreditNote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_credit_notes_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_credit_notes_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers(
        &self,
        args: &GetCustomersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_builder(
            &self.http_client,
            &args.created,
            &args.email,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.test_clock,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers search.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_search(
        &self,
        args: &GetCustomersSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_search_builder(
            &self.http_client,
            &args.expand,
            &args.limit,
            &args.page,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get customers customer.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer(
        &self,
        args: &GetCustomersCustomerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_builder(
            &self.http_client,
            &args.customer,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer balance transactions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_balance_transactions(
        &self,
        args: &GetCustomersCustomerBalanceTransactionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_balance_transactions_builder(
            &self.http_client,
            &args.customer,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.invoice,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_balance_transactions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer balance transactions transaction.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_balance_transactions_transaction(
        &self,
        args: &GetCustomersCustomerBalanceTransactionsTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerBalanceTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_balance_transactions_transaction_builder(
            &self.http_client,
            &args.customer,
            &args.transaction,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_balance_transactions_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer bank accounts.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_bank_accounts(
        &self,
        args: &GetCustomersCustomerBankAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_bank_accounts_builder(
            &self.http_client,
            &args.customer,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_bank_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the PaymentSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_bank_accounts(
        &self,
        args: &PostCustomersCustomerBankAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentSource, ProviderError<ApiError>>,
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

    /// Get customers customer bank accounts id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_bank_accounts_id(
        &self,
        args: &GetCustomersCustomerBankAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BankAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_bank_accounts_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_bank_accounts_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer cards.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_cards(
        &self,
        args: &GetCustomersCustomerCardsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_cards_builder(
            &self.http_client,
            &args.customer,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_cards_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the PaymentSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_cards(
        &self,
        args: &PostCustomersCustomerCardsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentSource, ProviderError<ApiError>>,
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

    /// Get customers customer cards id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Card result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_cards_id(
        &self,
        args: &GetCustomersCustomerCardsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Card, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_cards_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_cards_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer cash balance.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_cash_balance(
        &self,
        args: &GetCustomersCustomerCashBalanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CashBalance, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_cash_balance_builder(
            &self.http_client,
            &args.customer,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_cash_balance_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer cash balance transactions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_cash_balance_transactions(
        &self,
        args: &GetCustomersCustomerCashBalanceTransactionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_cash_balance_transactions_builder(
            &self.http_client,
            &args.customer,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_cash_balance_transactions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get customers customer cash balance transactions transaction.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_cash_balance_transactions_transaction(
        &self,
        args: &GetCustomersCustomerCashBalanceTransactionsTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerCashBalanceTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_cash_balance_transactions_transaction_builder(
            &self.http_client,
            &args.customer,
            &args.transaction,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_cash_balance_transactions_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get customers customer discount.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Discount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_discount(
        &self,
        args: &GetCustomersCustomerDiscountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Discount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_discount_builder(
            &self.http_client,
            &args.customer,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_discount_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer payment methods.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_payment_methods(
        &self,
        args: &GetCustomersCustomerPaymentMethodsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_payment_methods_builder(
            &self.http_client,
            &args.customer,
            &args.allow_redisplay,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_payment_methods_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get customers customer payment methods payment method.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_payment_methods_payment_method(
        &self,
        args: &GetCustomersCustomerPaymentMethodsPaymentMethodArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethod, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_payment_methods_payment_method_builder(
            &self.http_client,
            &args.customer,
            &args.payment_method,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_payment_methods_payment_method_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get customers customer sources.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_sources(
        &self,
        args: &GetCustomersCustomerSourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_sources_builder(
            &self.http_client,
            &args.customer,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.object,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_sources_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the PaymentSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_customers_customer_sources(
        &self,
        args: &PostCustomersCustomerSourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentSource, ProviderError<ApiError>>,
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

    /// Get customers customer sources id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_sources_id(
        &self,
        args: &GetCustomersCustomerSourcesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentSource, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_sources_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_sources_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer subscriptions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_subscriptions(
        &self,
        args: &GetCustomersCustomerSubscriptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_subscriptions_builder(
            &self.http_client,
            &args.customer,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_subscriptions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer subscriptions subscription exposed id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_subscriptions_subscription_exposed_id(
        &self,
        args: &GetCustomersCustomerSubscriptionsSubscriptionExposedIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_subscriptions_subscription_exposed_id_builder(
            &self.http_client,
            &args.customer,
            &args.subscription_exposed_id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_subscriptions_subscription_exposed_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer subscriptions subscription exposed id discount.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Discount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_subscriptions_subscription_exposed_id_discount(
        &self,
        args: &GetCustomersCustomerSubscriptionsSubscriptionExposedIdDiscountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Discount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_subscriptions_subscription_exposed_id_discount_builder(
            &self.http_client,
            &args.customer,
            &args.subscription_exposed_id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_subscriptions_subscription_exposed_id_discount_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer tax ids.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_tax_ids(
        &self,
        args: &GetCustomersCustomerTaxIdsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_tax_ids_builder(
            &self.http_client,
            &args.customer,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_tax_ids_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get customers customer tax ids id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_customers_customer_tax_ids_id(
        &self,
        args: &GetCustomersCustomerTaxIdsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxId, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_customers_customer_tax_ids_id_builder(
            &self.http_client,
            &args.customer,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_customers_customer_tax_ids_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get disputes.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_disputes(
        &self,
        args: &GetDisputesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_disputes_builder(
            &self.http_client,
            &args.charge,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.payment_intent,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_disputes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get disputes dispute.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_disputes_dispute(
        &self,
        args: &GetDisputesDisputeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_disputes_dispute_builder(
            &self.http_client,
            &args.dispute,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_disputes_dispute_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get entitlements active entitlements.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_entitlements_active_entitlements(
        &self,
        args: &GetEntitlementsActiveEntitlementsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_entitlements_active_entitlements_builder(
            &self.http_client,
            &args.customer,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_entitlements_active_entitlements_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get entitlements active entitlements id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntitlementsActiveEntitlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_entitlements_active_entitlements_id(
        &self,
        args: &GetEntitlementsActiveEntitlementsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntitlementsActiveEntitlement, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_entitlements_active_entitlements_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_entitlements_active_entitlements_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get entitlements features.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_entitlements_features(
        &self,
        args: &GetEntitlementsFeaturesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_entitlements_features_builder(
            &self.http_client,
            &args.archived,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.lookup_key,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_entitlements_features_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get entitlements features id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_entitlements_features_id(
        &self,
        args: &GetEntitlementsFeaturesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntitlementsFeature, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_entitlements_features_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_entitlements_features_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get events.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_events(
        &self,
        args: &GetEventsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_events_builder(
            &self.http_client,
            &args.created,
            &args.delivery_success,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.type_rs,
            &args.types,
        )
        .map_err(ProviderError::Api)?;

        let task = get_events_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get events id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Event result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_events_id(
        &self,
        args: &GetEventsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Event, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_events_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_events_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get exchange rates.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_exchange_rates(
        &self,
        args: &GetExchangeRatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_exchange_rates_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_exchange_rates_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get exchange rates rate id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExchangeRate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_exchange_rates_rate_id(
        &self,
        args: &GetExchangeRatesRateIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExchangeRate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_exchange_rates_rate_id_builder(
            &self.http_client,
            &args.rate_id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_exchange_rates_rate_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the ExternalAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_external_accounts_id(
        &self,
        args: &PostExternalAccountsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalAccount, ProviderError<ApiError>>,
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

    /// Get file links.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_file_links(
        &self,
        args: &GetFileLinksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_file_links_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.expired,
            &args.file,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_file_links_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get file links link.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_file_links_link(
        &self,
        args: &GetFileLinksLinkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FileLink, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_file_links_link_builder(
            &self.http_client,
            &args.link,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_file_links_link_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get files.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_files(
        &self,
        args: &GetFilesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_files_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.purpose,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_files_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get files file.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_files_file(
        &self,
        args: &GetFilesFileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<File, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_files_file_builder(
            &self.http_client,
            &args.file,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_files_file_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get financial connections accounts.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_financial_connections_accounts(
        &self,
        args: &GetFinancialConnectionsAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_financial_connections_accounts_builder(
            &self.http_client,
            &args.account_holder,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.session,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_financial_connections_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get financial connections accounts account.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_financial_connections_accounts_account(
        &self,
        args: &GetFinancialConnectionsAccountsAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_financial_connections_accounts_account_builder(
            &self.http_client,
            &args.account,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_financial_connections_accounts_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get financial connections accounts account owners.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_financial_connections_accounts_account_owners(
        &self,
        args: &GetFinancialConnectionsAccountsAccountOwnersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_financial_connections_accounts_account_owners_builder(
            &self.http_client,
            &args.account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.ownership,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_financial_connections_accounts_account_owners_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get financial connections sessions session.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_financial_connections_sessions_session(
        &self,
        args: &GetFinancialConnectionsSessionsSessionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_financial_connections_sessions_session_builder(
            &self.http_client,
            &args.session,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_financial_connections_sessions_session_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get financial connections transactions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_financial_connections_transactions(
        &self,
        args: &GetFinancialConnectionsTransactionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_financial_connections_transactions_builder(
            &self.http_client,
            &args.account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.transacted_at,
            &args.transaction_refresh,
        )
        .map_err(ProviderError::Api)?;

        let task = get_financial_connections_transactions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get financial connections transactions transaction.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinancialConnectionsTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_financial_connections_transactions_transaction(
        &self,
        args: &GetFinancialConnectionsTransactionsTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_financial_connections_transactions_transaction_builder(
            &self.http_client,
            &args.transaction,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_financial_connections_transactions_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get forwarding requests.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_forwarding_requests(
        &self,
        args: &GetForwardingRequestsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_forwarding_requests_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_forwarding_requests_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get forwarding requests id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_forwarding_requests_id(
        &self,
        args: &GetForwardingRequestsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ForwardingRequest, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_forwarding_requests_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_forwarding_requests_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get identity verification reports.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_identity_verification_reports(
        &self,
        args: &GetIdentityVerificationReportsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_identity_verification_reports_builder(
            &self.http_client,
            &args.client_reference_id,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.type_rs,
            &args.verification_session,
        )
        .map_err(ProviderError::Api)?;

        let task = get_identity_verification_reports_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get identity verification reports report.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentityVerificationReport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_identity_verification_reports_report(
        &self,
        args: &GetIdentityVerificationReportsReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentityVerificationReport, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_identity_verification_reports_report_builder(
            &self.http_client,
            &args.report,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_identity_verification_reports_report_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get identity verification sessions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_identity_verification_sessions(
        &self,
        args: &GetIdentityVerificationSessionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_identity_verification_sessions_builder(
            &self.http_client,
            &args.client_reference_id,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.related_customer,
            &args.related_customer_account,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_identity_verification_sessions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get identity verification sessions session.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_identity_verification_sessions_session(
        &self,
        args: &GetIdentityVerificationSessionsSessionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentityVerificationSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_identity_verification_sessions_session_builder(
            &self.http_client,
            &args.session,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_identity_verification_sessions_session_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get invoice payments.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_invoice_payments(
        &self,
        args: &GetInvoicePaymentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoice_payments_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.invoice,
            &args.limit,
            &args.payment,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoice_payments_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get invoice payments invoice payment.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InvoicePayment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_invoice_payments_invoice_payment(
        &self,
        args: &GetInvoicePaymentsInvoicePaymentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InvoicePayment, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoice_payments_invoice_payment_builder(
            &self.http_client,
            &args.invoice_payment,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoice_payments_invoice_payment_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get invoice rendering templates.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_invoice_rendering_templates(
        &self,
        args: &GetInvoiceRenderingTemplatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoice_rendering_templates_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoice_rendering_templates_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get invoice rendering templates template.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_invoice_rendering_templates_template(
        &self,
        args: &GetInvoiceRenderingTemplatesTemplateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InvoiceRenderingTemplate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoice_rendering_templates_template_builder(
            &self.http_client,
            &args.template,
            &args.expand,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoice_rendering_templates_template_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoice rendering templates template archive.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post invoice rendering templates template unarchive.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get invoiceitems.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_invoiceitems(
        &self,
        args: &GetInvoiceitemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoiceitems_builder(
            &self.http_client,
            &args.created,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.invoice,
            &args.limit,
            &args.pending,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoiceitems_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get invoiceitems invoiceitem.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_invoiceitems_invoiceitem(
        &self,
        args: &GetInvoiceitemsInvoiceitemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoiceitem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoiceitems_invoiceitem_builder(
            &self.http_client,
            &args.invoiceitem,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoiceitems_invoiceitem_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get invoices.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_invoices(
        &self,
        args: &GetInvoicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoices_builder(
            &self.http_client,
            &args.collection_method,
            &args.created,
            &args.customer,
            &args.customer_account,
            &args.due_date,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoices_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get invoices search.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_invoices_search(
        &self,
        args: &GetInvoicesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoices_search_builder(
            &self.http_client,
            &args.expand,
            &args.limit,
            &args.page,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoices_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get invoices invoice.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_invoices_invoice(
        &self,
        args: &GetInvoicesInvoiceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Invoice, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoices_invoice_builder(
            &self.http_client,
            &args.invoice,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoices_invoice_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get invoices invoice lines.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_invoices_invoice_lines(
        &self,
        args: &GetInvoicesInvoiceLinesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoices_invoice_lines_builder(
            &self.http_client,
            &args.invoice,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoices_invoice_lines_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing authorizations.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_authorizations(
        &self,
        args: &GetIssuingAuthorizationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_authorizations_builder(
            &self.http_client,
            &args.card,
            &args.cardholder,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_authorizations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get issuing authorizations authorization.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_authorizations_authorization(
        &self,
        args: &GetIssuingAuthorizationsAuthorizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingAuthorization, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_authorizations_authorization_builder(
            &self.http_client,
            &args.authorization,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_authorizations_authorization_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing cardholders.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_cardholders(
        &self,
        args: &GetIssuingCardholdersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_cardholders_builder(
            &self.http_client,
            &args.created,
            &args.email,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.phone_number,
            &args.starting_after,
            &args.status,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_cardholders_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing cardholders cardholder.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_cardholders_cardholder(
        &self,
        args: &GetIssuingCardholdersCardholderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCardholder, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_cardholders_cardholder_builder(
            &self.http_client,
            &args.cardholder,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_cardholders_cardholder_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing cards.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_cards(
        &self,
        args: &GetIssuingCardsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_cards_builder(
            &self.http_client,
            &args.cardholder,
            &args.created,
            &args.ending_before,
            &args.exp_month,
            &args.exp_year,
            &args.expand,
            &args.last4,
            &args.limit,
            &args.personalization_design,
            &args.starting_after,
            &args.status,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_cards_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing cards card.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_cards_card(
        &self,
        args: &GetIssuingCardsCardArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingCard, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_cards_card_builder(
            &self.http_client,
            &args.card,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_cards_card_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing disputes.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_disputes(
        &self,
        args: &GetIssuingDisputesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_disputes_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
            &args.transaction,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_disputes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing disputes dispute.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_disputes_dispute(
        &self,
        args: &GetIssuingDisputesDisputeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingDispute, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_disputes_dispute_builder(
            &self.http_client,
            &args.dispute,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_disputes_dispute_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing personalization designs.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_personalization_designs(
        &self,
        args: &GetIssuingPersonalizationDesignsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_personalization_designs_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.lookup_keys,
            &args.preferences,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_personalization_designs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing personalization designs personalization design.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_personalization_designs_personalization_design(
        &self,
        args: &GetIssuingPersonalizationDesignsPersonalizationDesignArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingPersonalizationDesign, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_personalization_designs_personalization_design_builder(
            &self.http_client,
            &args.personalization_design,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_personalization_designs_personalization_design_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing physical bundles.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_physical_bundles(
        &self,
        args: &GetIssuingPhysicalBundlesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_physical_bundles_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_physical_bundles_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get issuing physical bundles physical bundle.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuingPhysicalBundle result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_physical_bundles_physical_bundle(
        &self,
        args: &GetIssuingPhysicalBundlesPhysicalBundleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingPhysicalBundle, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_physical_bundles_physical_bundle_builder(
            &self.http_client,
            &args.physical_bundle,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_physical_bundles_physical_bundle_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get issuing settlements settlement.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_settlements_settlement(
        &self,
        args: &GetIssuingSettlementsSettlementArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingSettlement, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_settlements_settlement_builder(
            &self.http_client,
            &args.settlement,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_settlements_settlement_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing tokens.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_tokens(
        &self,
        args: &GetIssuingTokensArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_tokens_builder(
            &self.http_client,
            &args.card,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_tokens_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get issuing tokens token.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_tokens_token(
        &self,
        args: &GetIssuingTokensTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingToken, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_tokens_token_builder(
            &self.http_client,
            &args.token,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_tokens_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get issuing transactions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_transactions(
        &self,
        args: &GetIssuingTransactionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_transactions_builder(
            &self.http_client,
            &args.card,
            &args.cardholder,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_transactions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get issuing transactions transaction.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_issuing_transactions_transaction(
        &self,
        args: &GetIssuingTransactionsTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuingTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_issuing_transactions_transaction_builder(
            &self.http_client,
            &args.transaction,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_issuing_transactions_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get link account sessions session.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_link_account_sessions_session(
        &self,
        args: &GetLinkAccountSessionsSessionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsSession, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_link_account_sessions_session_builder(
            &self.http_client,
            &args.session,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_link_account_sessions_session_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get linked accounts.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_linked_accounts(
        &self,
        args: &GetLinkedAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_linked_accounts_builder(
            &self.http_client,
            &args.account_holder,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.session,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_linked_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get linked accounts account.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_linked_accounts_account(
        &self,
        args: &GetLinkedAccountsAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinancialConnectionsAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_linked_accounts_account_builder(
            &self.http_client,
            &args.account,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_linked_accounts_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get linked accounts account owners.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_linked_accounts_account_owners(
        &self,
        args: &GetLinkedAccountsAccountOwnersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_linked_accounts_account_owners_builder(
            &self.http_client,
            &args.account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.ownership,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_linked_accounts_account_owners_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get mandates mandate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Mandate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_mandates_mandate(
        &self,
        args: &GetMandatesMandateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Mandate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_mandates_mandate_builder(
            &self.http_client,
            &args.mandate,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_mandates_mandate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get payment attempt records.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_attempt_records(
        &self,
        args: &GetPaymentAttemptRecordsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_attempt_records_builder(
            &self.http_client,
            &args.expand,
            &args.limit,
            &args.payment_record,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_attempt_records_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get payment attempt records id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PaymentAttemptRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_attempt_records_id(
        &self,
        args: &GetPaymentAttemptRecordsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentAttemptRecord, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_attempt_records_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_attempt_records_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get payment intents.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_intents(
        &self,
        args: &GetPaymentIntentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_intents_builder(
            &self.http_client,
            &args.created,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_intents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment intents search.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_intents_search(
        &self,
        args: &GetPaymentIntentsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_intents_search_builder(
            &self.http_client,
            &args.expand,
            &args.limit,
            &args.page,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_intents_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get payment intents intent.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_intents_intent(
        &self,
        args: &GetPaymentIntentsIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_intents_intent_builder(
            &self.http_client,
            &args.intent,
            &args.client_secret,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_intents_intent_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment intents intent amount details line items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_intents_intent_amount_details_line_items(
        &self,
        args: &GetPaymentIntentsIntentAmountDetailsLineItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_intents_intent_amount_details_line_items_builder(
            &self.http_client,
            &args.intent,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_intents_intent_amount_details_line_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment links.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_links(
        &self,
        args: &GetPaymentLinksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_links_builder(
            &self.http_client,
            &args.active,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_links_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment links payment link.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_links_payment_link(
        &self,
        args: &GetPaymentLinksPaymentLinkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentLink, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_links_payment_link_builder(
            &self.http_client,
            &args.payment_link,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_links_payment_link_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment links payment link line items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_links_payment_link_line_items(
        &self,
        args: &GetPaymentLinksPaymentLinkLineItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_links_payment_link_line_items_builder(
            &self.http_client,
            &args.payment_link,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_links_payment_link_line_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get payment method configurations.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_method_configurations(
        &self,
        args: &GetPaymentMethodConfigurationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_method_configurations_builder(
            &self.http_client,
            &args.application,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_method_configurations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment method configurations configuration.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_method_configurations_configuration(
        &self,
        args: &GetPaymentMethodConfigurationsConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethodConfiguration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_method_configurations_configuration_builder(
            &self.http_client,
            &args.configuration,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_method_configurations_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment method domains.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_method_domains(
        &self,
        args: &GetPaymentMethodDomainsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_method_domains_builder(
            &self.http_client,
            &args.domain_name,
            &args.enabled,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_method_domains_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment method domains payment method domain.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_method_domains_payment_method_domain(
        &self,
        args: &GetPaymentMethodDomainsPaymentMethodDomainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethodDomain, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_method_domains_payment_method_domain_builder(
            &self.http_client,
            &args.payment_method_domain,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_method_domains_payment_method_domain_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get payment methods.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_methods(
        &self,
        args: &GetPaymentMethodsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_methods_builder(
            &self.http_client,
            &args.allow_redisplay,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_methods_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment methods payment method.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_methods_payment_method(
        &self,
        args: &GetPaymentMethodsPaymentMethodArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentMethod, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_methods_payment_method_builder(
            &self.http_client,
            &args.payment_method,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_methods_payment_method_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payment records id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payment_records_id(
        &self,
        args: &GetPaymentRecordsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PaymentRecord, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payment_records_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payment_records_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payouts.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payouts(
        &self,
        args: &GetPayoutsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payouts_builder(
            &self.http_client,
            &args.arrival_date,
            &args.created,
            &args.destination,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payouts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get payouts payout.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_payouts_payout(
        &self,
        args: &GetPayoutsPayoutArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Payout, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_payouts_payout_builder(
            &self.http_client,
            &args.payout,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_payouts_payout_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get plans.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_plans(
        &self,
        args: &GetPlansArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_plans_builder(
            &self.http_client,
            &args.active,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.product,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_plans_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get plans plan.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_plans_plan(
        &self,
        args: &GetPlansPlanArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Plan, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_plans_plan_builder(
            &self.http_client,
            &args.plan,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_plans_plan_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get prices.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_prices(
        &self,
        args: &GetPricesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_prices_builder(
            &self.http_client,
            &args.active,
            &args.created,
            &args.currency,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.lookup_keys,
            &args.product,
            &args.recurring,
            &args.starting_after,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = get_prices_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get prices search.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_prices_search(
        &self,
        args: &GetPricesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_prices_search_builder(
            &self.http_client,
            &args.expand,
            &args.limit,
            &args.page,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = get_prices_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get prices price.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_prices_price(
        &self,
        args: &GetPricesPriceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Price, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_prices_price_builder(
            &self.http_client,
            &args.price,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_prices_price_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get products.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_products(
        &self,
        args: &GetProductsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_products_builder(
            &self.http_client,
            &args.active,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.ids,
            &args.limit,
            &args.shippable,
            &args.starting_after,
            &args.url,
        )
        .map_err(ProviderError::Api)?;

        let task = get_products_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get products search.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_products_search(
        &self,
        args: &GetProductsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_products_search_builder(
            &self.http_client,
            &args.expand,
            &args.limit,
            &args.page,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = get_products_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get products id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_products_id(
        &self,
        args: &GetProductsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_products_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_products_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get products product features.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_products_product_features(
        &self,
        args: &GetProductsProductFeaturesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_products_product_features_builder(
            &self.http_client,
            &args.product,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_products_product_features_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get products product features id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_products_product_features_id(
        &self,
        args: &GetProductsProductFeaturesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductFeature, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_products_product_features_id_builder(
            &self.http_client,
            &args.id,
            &args.product,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_products_product_features_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get promotion codes.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_promotion_codes(
        &self,
        args: &GetPromotionCodesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_promotion_codes_builder(
            &self.http_client,
            &args.active,
            &args.code,
            &args.coupon,
            &args.created,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_promotion_codes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get promotion codes promotion code.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_promotion_codes_promotion_code(
        &self,
        args: &GetPromotionCodesPromotionCodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PromotionCode, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_promotion_codes_promotion_code_builder(
            &self.http_client,
            &args.promotion_code,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_promotion_codes_promotion_code_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get quotes.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_quotes(
        &self,
        args: &GetQuotesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_quotes_builder(
            &self.http_client,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
            &args.test_clock,
        )
        .map_err(ProviderError::Api)?;

        let task = get_quotes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get quotes quote.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_quotes_quote(
        &self,
        args: &GetQuotesQuoteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Quote, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_quotes_quote_builder(
            &self.http_client,
            &args.quote,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_quotes_quote_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get quotes quote computed upfront line items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_quotes_quote_computed_upfront_line_items(
        &self,
        args: &GetQuotesQuoteComputedUpfrontLineItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_quotes_quote_computed_upfront_line_items_builder(
            &self.http_client,
            &args.quote,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_quotes_quote_computed_upfront_line_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get quotes quote line items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_quotes_quote_line_items(
        &self,
        args: &GetQuotesQuoteLineItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_quotes_quote_line_items_builder(
            &self.http_client,
            &args.quote,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_quotes_quote_line_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get quotes quote pdf.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_quotes_quote_pdf(
        &self,
        args: &GetQuotesQuotePdfArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_quotes_quote_pdf_builder(
            &self.http_client,
            &args.quote,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_quotes_quote_pdf_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get radar early fraud warnings.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_radar_early_fraud_warnings(
        &self,
        args: &GetRadarEarlyFraudWarningsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_radar_early_fraud_warnings_builder(
            &self.http_client,
            &args.charge,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.payment_intent,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_radar_early_fraud_warnings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get radar early fraud warnings early fraud warning.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RadarEarlyFraudWarning result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_radar_early_fraud_warnings_early_fraud_warning(
        &self,
        args: &GetRadarEarlyFraudWarningsEarlyFraudWarningArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RadarEarlyFraudWarning, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_radar_early_fraud_warnings_early_fraud_warning_builder(
            &self.http_client,
            &args.early_fraud_warning,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_radar_early_fraud_warnings_early_fraud_warning_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get radar value list items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_radar_value_list_items(
        &self,
        args: &GetRadarValueListItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_radar_value_list_items_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.value,
            &args.value_list,
        )
        .map_err(ProviderError::Api)?;

        let task = get_radar_value_list_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post radar value list items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get radar value list items item.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_radar_value_list_items_item(
        &self,
        args: &GetRadarValueListItemsItemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RadarValueListItem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_radar_value_list_items_item_builder(
            &self.http_client,
            &args.item,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_radar_value_list_items_item_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete radar value list items item.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get radar value lists.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_radar_value_lists(
        &self,
        args: &GetRadarValueListsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_radar_value_lists_builder(
            &self.http_client,
            &args.alias,
            &args.contains,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_radar_value_lists_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post radar value lists.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get radar value lists value list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_radar_value_lists_value_list(
        &self,
        args: &GetRadarValueListsValueListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RadarValueList, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_radar_value_lists_value_list_builder(
            &self.http_client,
            &args.value_list,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_radar_value_lists_value_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post radar value lists value list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete radar value lists value list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get refunds.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_refunds(
        &self,
        args: &GetRefundsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_refunds_builder(
            &self.http_client,
            &args.charge,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.payment_intent,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_refunds_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get refunds refund.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_refunds_refund(
        &self,
        args: &GetRefundsRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Refund, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_refunds_refund_builder(
            &self.http_client,
            &args.refund,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_refunds_refund_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get reporting report runs.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_reporting_report_runs(
        &self,
        args: &GetReportingReportRunsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_reporting_report_runs_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_reporting_report_runs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get reporting report runs report run.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_reporting_report_runs_report_run(
        &self,
        args: &GetReportingReportRunsReportRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportingReportRun, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_reporting_report_runs_report_run_builder(
            &self.http_client,
            &args.report_run,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_reporting_report_runs_report_run_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get reporting report types.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_reporting_report_types(
        &self,
        args: &GetReportingReportTypesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_reporting_report_types_builder(
            &self.http_client,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_reporting_report_types_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get reporting report types report type.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportingReportType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_reporting_report_types_report_type(
        &self,
        args: &GetReportingReportTypesReportTypeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportingReportType, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_reporting_report_types_report_type_builder(
            &self.http_client,
            &args.report_type,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_reporting_report_types_report_type_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get reviews.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_reviews(
        &self,
        args: &GetReviewsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_reviews_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_reviews_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get reviews review.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_reviews_review(
        &self,
        args: &GetReviewsReviewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Review, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_reviews_review_builder(
            &self.http_client,
            &args.review,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_reviews_review_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get setup attempts.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_setup_attempts(
        &self,
        args: &GetSetupAttemptsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_setup_attempts_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.setup_intent,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_setup_attempts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get setup intents.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_setup_intents(
        &self,
        args: &GetSetupIntentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_setup_intents_builder(
            &self.http_client,
            &args.attach_to_self,
            &args.created,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.payment_method,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_setup_intents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get setup intents intent.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_setup_intents_intent(
        &self,
        args: &GetSetupIntentsIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetupIntent, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_setup_intents_intent_builder(
            &self.http_client,
            &args.intent,
            &args.client_secret,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_setup_intents_intent_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get shipping rates.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_shipping_rates(
        &self,
        args: &GetShippingRatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_shipping_rates_builder(
            &self.http_client,
            &args.active,
            &args.created,
            &args.currency,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_shipping_rates_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get shipping rates shipping rate token.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_shipping_rates_shipping_rate_token(
        &self,
        args: &GetShippingRatesShippingRateTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingRate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_shipping_rates_shipping_rate_token_builder(
            &self.http_client,
            &args.shipping_rate_token,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_shipping_rates_shipping_rate_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get sigma scheduled query runs.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_sigma_scheduled_query_runs(
        &self,
        args: &GetSigmaScheduledQueryRunsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_sigma_scheduled_query_runs_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_sigma_scheduled_query_runs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get sigma scheduled query runs scheduled query run.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ScheduledQueryRun result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_sigma_scheduled_query_runs_scheduled_query_run(
        &self,
        args: &GetSigmaScheduledQueryRunsScheduledQueryRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ScheduledQueryRun, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_sigma_scheduled_query_runs_scheduled_query_run_builder(
            &self.http_client,
            &args.scheduled_query_run,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_sigma_scheduled_query_runs_scheduled_query_run_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get sources source.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_sources_source(
        &self,
        args: &GetSourcesSourceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Source, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_sources_source_builder(
            &self.http_client,
            &args.source,
            &args.client_secret,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_sources_source_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get sources source mandate notifications mandate notification.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SourceMandateNotification result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_sources_source_mandate_notifications_mandate_notification(
        &self,
        args: &GetSourcesSourceMandateNotificationsMandateNotificationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SourceMandateNotification, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_sources_source_mandate_notifications_mandate_notification_builder(
            &self.http_client,
            &args.mandate_notification,
            &args.source,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_sources_source_mandate_notifications_mandate_notification_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get sources source source transactions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_sources_source_source_transactions(
        &self,
        args: &GetSourcesSourceSourceTransactionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_sources_source_source_transactions_builder(
            &self.http_client,
            &args.source,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_sources_source_source_transactions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get sources source source transactions source transaction.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SourceTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_sources_source_source_transactions_source_transaction(
        &self,
        args: &GetSourcesSourceSourceTransactionsSourceTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SourceTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_sources_source_source_transactions_source_transaction_builder(
            &self.http_client,
            &args.source,
            &args.source_transaction,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_sources_source_source_transactions_source_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get subscription items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_subscription_items(
        &self,
        args: &GetSubscriptionItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_subscription_items_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = get_subscription_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get subscription items item.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_subscription_items_item(
        &self,
        args: &GetSubscriptionItemsItemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionItem, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_subscription_items_item_builder(
            &self.http_client,
            &args.item,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_subscription_items_item_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get subscription schedules.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_subscription_schedules(
        &self,
        args: &GetSubscriptionSchedulesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_subscription_schedules_builder(
            &self.http_client,
            &args.canceled_at,
            &args.completed_at,
            &args.created,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.released_at,
            &args.scheduled,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_subscription_schedules_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get subscription schedules schedule.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_subscription_schedules_schedule(
        &self,
        args: &GetSubscriptionSchedulesScheduleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionSchedule, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_subscription_schedules_schedule_builder(
            &self.http_client,
            &args.schedule,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_subscription_schedules_schedule_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get subscriptions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_subscriptions(
        &self,
        args: &GetSubscriptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_subscriptions_builder(
            &self.http_client,
            &args.automatic_tax,
            &args.collection_method,
            &args.created,
            &args.current_period_end,
            &args.current_period_start,
            &args.customer,
            &args.customer_account,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.price,
            &args.starting_after,
            &args.status,
            &args.test_clock,
        )
        .map_err(ProviderError::Api)?;

        let task = get_subscriptions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get subscriptions search.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_subscriptions_search(
        &self,
        args: &GetSubscriptionsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_subscriptions_search_builder(
            &self.http_client,
            &args.expand,
            &args.limit,
            &args.page,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = get_subscriptions_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get subscriptions subscription exposed id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_subscriptions_subscription_exposed_id(
        &self,
        args: &GetSubscriptionsSubscriptionExposedIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_subscriptions_subscription_exposed_id_builder(
            &self.http_client,
            &args.subscription_exposed_id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_subscriptions_subscription_exposed_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get tax associations find.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxAssociation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_associations_find(
        &self,
        args: &GetTaxAssociationsFindArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxAssociation, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_associations_find_builder(
            &self.http_client,
            &args.expand,
            &args.payment_intent,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_associations_find_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get tax calculations calculation.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_calculations_calculation(
        &self,
        args: &GetTaxCalculationsCalculationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxCalculation, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_calculations_calculation_builder(
            &self.http_client,
            &args.calculation,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_calculations_calculation_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get tax calculations calculation line items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_calculations_calculation_line_items(
        &self,
        args: &GetTaxCalculationsCalculationLineItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_calculations_calculation_line_items_builder(
            &self.http_client,
            &args.calculation,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_calculations_calculation_line_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get tax registrations.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_registrations(
        &self,
        args: &GetTaxRegistrationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_registrations_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_registrations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get tax registrations id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_registrations_id(
        &self,
        args: &GetTaxRegistrationsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxRegistration, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_registrations_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_registrations_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get tax settings.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_settings(
        &self,
        args: &GetTaxSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxSettings, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_settings_builder(
            &self.http_client,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get tax transactions transaction.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_transactions_transaction(
        &self,
        args: &GetTaxTransactionsTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_transactions_transaction_builder(
            &self.http_client,
            &args.transaction,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_transactions_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get tax transactions transaction line items.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_transactions_transaction_line_items(
        &self,
        args: &GetTaxTransactionsTransactionLineItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_transactions_transaction_line_items_builder(
            &self.http_client,
            &args.transaction,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_transactions_transaction_line_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get tax codes.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_codes(
        &self,
        args: &GetTaxCodesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_codes_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_codes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get tax codes id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaxCode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_codes_id(
        &self,
        args: &GetTaxCodesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxCode, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_codes_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_codes_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get tax ids.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_ids(
        &self,
        args: &GetTaxIdsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_ids_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.owner,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_ids_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get tax ids id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_ids_id(
        &self,
        args: &GetTaxIdsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxId, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_ids_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_ids_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get tax rates.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_rates(
        &self,
        args: &GetTaxRatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_rates_builder(
            &self.http_client,
            &args.active,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.inclusive,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_rates_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get tax rates tax rate.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tax_rates_tax_rate(
        &self,
        args: &GetTaxRatesTaxRateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaxRate, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tax_rates_tax_rate_builder(
            &self.http_client,
            &args.tax_rate,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tax_rates_tax_rate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post tax rates tax rate.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get terminal configurations.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_terminal_configurations(
        &self,
        args: &GetTerminalConfigurationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_terminal_configurations_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.is_account_default,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_terminal_configurations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get terminal configurations configuration.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_terminal_configurations_configuration(
        &self,
        args: &GetTerminalConfigurationsConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_terminal_configurations_configuration_builder(
            &self.http_client,
            &args.configuration,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_terminal_configurations_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get terminal locations.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_terminal_locations(
        &self,
        args: &GetTerminalLocationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_terminal_locations_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_terminal_locations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get terminal locations location.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_terminal_locations_location(
        &self,
        args: &GetTerminalLocationsLocationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_terminal_locations_location_builder(
            &self.http_client,
            &args.location,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_terminal_locations_location_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get terminal readers.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_terminal_readers(
        &self,
        args: &GetTerminalReadersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_terminal_readers_builder(
            &self.http_client,
            &args.device_type,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.location,
            &args.serial_number,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_terminal_readers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get terminal readers reader.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_terminal_readers_reader(
        &self,
        args: &GetTerminalReadersReaderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_terminal_readers_reader_builder(
            &self.http_client,
            &args.reader,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_terminal_readers_reader_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers customers customer fund cash balance.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization capture.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization expire.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization finalize amount.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization fraud challenges respond.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization increment.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing authorizations authorization reverse.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping deliver.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping fail.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping return.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping ship.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers issuing cards card shipping submit.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers refunds refund expire.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers terminal readers reader present payment method.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers terminal readers reader succeed input collection.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers terminal readers reader timeout input collection.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get test helpers test clocks.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_test_helpers_test_clocks(
        &self,
        args: &GetTestHelpersTestClocksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_test_helpers_test_clocks_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_test_helpers_test_clocks_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers test clocks.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get test helpers test clocks test clock.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_test_helpers_test_clocks_test_clock(
        &self,
        args: &GetTestHelpersTestClocksTestClockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestHelpersTestClock, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_test_helpers_test_clocks_test_clock_builder(
            &self.http_client,
            &args.test_clock,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_test_helpers_test_clocks_test_clock_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury inbound transfers id fail.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury inbound transfers id return.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury inbound transfers id succeed.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound payments id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound payments id fail.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound payments id post.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound payments id return.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound transfers outbound transfer.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound transfers outbound transfer fail.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound transfers outbound transfer post.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury outbound transfers outbound transfer return.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury received credits.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post test helpers treasury received debits.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get tokens token.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_tokens_token(
        &self,
        args: &GetTokensTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Token, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_tokens_token_builder(
            &self.http_client,
            &args.token,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_tokens_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get topups.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_topups(
        &self,
        args: &GetTopupsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_topups_builder(
            &self.http_client,
            &args.amount,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_topups_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get topups topup.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_topups_topup(
        &self,
        args: &GetTopupsTopupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topup, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_topups_topup_builder(
            &self.http_client,
            &args.topup,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_topups_topup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get transfers.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_transfers(
        &self,
        args: &GetTransfersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_transfers_builder(
            &self.http_client,
            &args.created,
            &args.destination,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.transfer_group,
        )
        .map_err(ProviderError::Api)?;

        let task = get_transfers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get transfers id reversals.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_transfers_id_reversals(
        &self,
        args: &GetTransfersIdReversalsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_transfers_id_reversals_builder(
            &self.http_client,
            &args.id,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_transfers_id_reversals_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get transfers transfer.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_transfers_transfer(
        &self,
        args: &GetTransfersTransferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Transfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_transfers_transfer_builder(
            &self.http_client,
            &args.transfer,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_transfers_transfer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get transfers transfer reversals id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_transfers_transfer_reversals_id(
        &self,
        args: &GetTransfersTransferReversalsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferReversal, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_transfers_transfer_reversals_id_builder(
            &self.http_client,
            &args.id,
            &args.transfer,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_transfers_transfer_reversals_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury credit reversals.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_credit_reversals(
        &self,
        args: &GetTreasuryCreditReversalsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_credit_reversals_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.financial_account,
            &args.limit,
            &args.received_credit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_credit_reversals_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury credit reversals credit reversal.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_credit_reversals_credit_reversal(
        &self,
        args: &GetTreasuryCreditReversalsCreditReversalArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryCreditReversal, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_credit_reversals_credit_reversal_builder(
            &self.http_client,
            &args.credit_reversal,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_credit_reversals_credit_reversal_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get treasury debit reversals.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_debit_reversals(
        &self,
        args: &GetTreasuryDebitReversalsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_debit_reversals_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.financial_account,
            &args.limit,
            &args.received_debit,
            &args.resolution,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_debit_reversals_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury debit reversals debit reversal.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_debit_reversals_debit_reversal(
        &self,
        args: &GetTreasuryDebitReversalsDebitReversalArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryDebitReversal, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_debit_reversals_debit_reversal_builder(
            &self.http_client,
            &args.debit_reversal,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_debit_reversals_debit_reversal_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get treasury financial accounts.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_financial_accounts(
        &self,
        args: &GetTreasuryFinancialAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_financial_accounts_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_financial_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury financial accounts financial account.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_financial_accounts_financial_account(
        &self,
        args: &GetTreasuryFinancialAccountsFinancialAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryFinancialAccount, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_financial_accounts_financial_account_builder(
            &self.http_client,
            &args.financial_account,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_financial_accounts_financial_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury financial accounts financial account features.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_financial_accounts_financial_account_features(
        &self,
        args: &GetTreasuryFinancialAccountsFinancialAccountFeaturesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryFinancialAccountFeatures, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_financial_accounts_financial_account_features_builder(
            &self.http_client,
            &args.financial_account,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_financial_accounts_financial_account_features_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury inbound transfers.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_inbound_transfers(
        &self,
        args: &GetTreasuryInboundTransfersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_inbound_transfers_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.financial_account,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_inbound_transfers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury inbound transfers id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_inbound_transfers_id(
        &self,
        args: &GetTreasuryInboundTransfersIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryInboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_inbound_transfers_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_inbound_transfers_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury outbound payments.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_outbound_payments(
        &self,
        args: &GetTreasuryOutboundPaymentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_outbound_payments_builder(
            &self.http_client,
            &args.created,
            &args.customer,
            &args.ending_before,
            &args.expand,
            &args.financial_account,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_outbound_payments_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury outbound payments id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_outbound_payments_id(
        &self,
        args: &GetTreasuryOutboundPaymentsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundPayment, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_outbound_payments_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_outbound_payments_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury outbound transfers.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_outbound_transfers(
        &self,
        args: &GetTreasuryOutboundTransfersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_outbound_transfers_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.financial_account,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_outbound_transfers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury outbound transfers outbound transfer.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_outbound_transfers_outbound_transfer(
        &self,
        args: &GetTreasuryOutboundTransfersOutboundTransferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryOutboundTransfer, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_outbound_transfers_outbound_transfer_builder(
            &self.http_client,
            &args.outbound_transfer,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_outbound_transfers_outbound_transfer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get treasury received credits.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_received_credits(
        &self,
        args: &GetTreasuryReceivedCreditsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_received_credits_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.financial_account,
            &args.limit,
            &args.linked_flows,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_received_credits_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get treasury received credits id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_received_credits_id(
        &self,
        args: &GetTreasuryReceivedCreditsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryReceivedCredit, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_received_credits_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_received_credits_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get treasury received debits.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_received_debits(
        &self,
        args: &GetTreasuryReceivedDebitsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_received_debits_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.financial_account,
            &args.limit,
            &args.starting_after,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_received_debits_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get treasury received debits id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_received_debits_id(
        &self,
        args: &GetTreasuryReceivedDebitsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryReceivedDebit, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_received_debits_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_received_debits_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get treasury transaction entries.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_transaction_entries(
        &self,
        args: &GetTreasuryTransactionEntriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_transaction_entries_builder(
            &self.http_client,
            &args.created,
            &args.effective_at,
            &args.ending_before,
            &args.expand,
            &args.financial_account,
            &args.limit,
            &args.order_by,
            &args.starting_after,
            &args.transaction,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_transaction_entries_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get treasury transaction entries id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryTransactionEntry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_transaction_entries_id(
        &self,
        args: &GetTreasuryTransactionEntriesIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryTransactionEntry, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_transaction_entries_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_transaction_entries_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get treasury transactions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_transactions(
        &self,
        args: &GetTreasuryTransactionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_transactions_builder(
            &self.http_client,
            &args.created,
            &args.ending_before,
            &args.expand,
            &args.financial_account,
            &args.limit,
            &args.order_by,
            &args.starting_after,
            &args.status,
            &args.status_transitions,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_transactions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get treasury transactions id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TreasuryTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_treasury_transactions_id(
        &self,
        args: &GetTreasuryTransactionsIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TreasuryTransaction, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_treasury_transactions_id_builder(
            &self.http_client,
            &args.id,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_treasury_transactions_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get webhook endpoints.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_webhook_endpoints(
        &self,
        args: &GetWebhookEndpointsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_webhook_endpoints_builder(
            &self.http_client,
            &args.ending_before,
            &args.expand,
            &args.limit,
            &args.starting_after,
        )
        .map_err(ProviderError::Api)?;

        let task = get_webhook_endpoints_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get webhook endpoints webhook endpoint.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_webhook_endpoints_webhook_endpoint(
        &self,
        args: &GetWebhookEndpointsWebhookEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebhookEndpoint, ProviderError<ApiError>>,
            P = crate::providers::stripe::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_webhook_endpoints_webhook_endpoint_builder(
            &self.http_client,
            &args.webhook_endpoint,
            &args.expand,
        )
        .map_err(ProviderError::Api)?;

        let task = get_webhook_endpoints_webhook_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
