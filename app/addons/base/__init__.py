"""
NexDL Addon Base Package
This package re-exports nexdl types for convenience.
"""
import nexdl

# Re-export for easier imports in addons
Addon = nexdl.Addon
DownloadItem = nexdl.DownloadItem
Context = nexdl.Context
HttpClient = nexdl.HttpClient
Storage = nexdl.Storage
