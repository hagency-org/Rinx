Source: project-robius/robius cca3cc3dc99d8fcb6c69a127cd28d47ba8f3e3eb, crates/file-picker (MIT).

Rinx adds a guarded Android save: revalidate account/instance authority on the save worker immediately before opening the selected content URI for writing. Ordinary picker calls retain their behavior. Cancel/error/completion removes the guard. Workspace dependencies are resolved to the original versions and pinned source.
