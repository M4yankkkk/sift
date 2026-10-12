# Engineering Architecture Notes: Sift Desktop

Date: October 2026
Author: Core Engineering

## Overview
Sift is a high-performance local desktop search utility for Ubuntu Linux.
The primary design goals are:
- Strictly offline indexing and search
- Deterministic sub-100ms warm queries
- Low memory footprint (under 100MB idle RAM)
- Modular search coordinator combining lexical and semantic results

## Indexing Lifecycle
1. Discovered in file scanner
2. Enqueued in persistent job queue
3. Text extraction and chunking
4. Tantivy keyword index insertion
5. Vector embedding generation (when enabled)
