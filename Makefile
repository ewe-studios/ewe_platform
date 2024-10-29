PACKAGES = ewe_trace ewe_async_utils ewe_channels ewe_domain ewe_devserver ewe_domain_server ewe_html ewe_html_macro ewe_mem ewe_routing ewe_spawn ewe_spawn ewe_template_macro ewe_templates ewe_temple ewe_watch_utils ewe_watchers ewe_web

test:
	$(foreach var,$(PACKAGES), cargo test --package $(var);)

publish:
	$(foreach var,$(PACKAGES), cargo publish --package $(var);)
