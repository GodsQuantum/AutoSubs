# Optional media corpus index

The helper creates local Markdown, SRT and word timestamps with an optional SQLite FTS5 search index from an operator-supplied video directory. It does not rewrite original media.

Configure source and endpoints in the operator environment, not in public source. Supported settings: AUTOSUBS_CORPUS_ROOT, AUTOSUBS_CORPUS_DB, AUTOSUBS_CORPUS_TMP_HOST, AUTOSUBS_CORPUS_STT_URL, AUTOSUBS_CORPUS_ALIGN_URL, AUTOSUBS_CORPUS_VAD_URL, AUTOSUBS_CORPUS_LXC_ID, AUTOSUBS_CORPUS_CONTAINER and AUTOSUBS_CORPUS_CONTAINER_ROOT.

Optional systemd service assumes an installed helper under /opt/autosubs/tools and a private environment file under /etc/autosubs. Never commit media, logs, account details or machine-specific settings.
