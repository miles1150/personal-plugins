# Waypost provenance

The service and bridge now live in this repository at `services/waypost`. The original Waypost repository remains available for full history. The service was imported from `miles1150/waypost@11c4668f63ff0ef2c67026ff12c061d6c7ac980f`; no production resources or data were moved by the import.

This skill and its references derive from Waypost commit `9f70cbf3779211544739ff7998b93895b8618944` under Apache-2.0 (see `UPSTREAM-LICENSE`). The pinned bridge tree `3cb14e1b29a3f628166cb054a4c780a29d041a92` is identical to the corresponding imported monorepo tree. Marketplace CI builds that committed tree locally, checks its identity and executable digest, and preserves the existing bridge provenance label for compatibility with the 0.3.0 launcher. The source plugin itself contains no binary; use the verified platform bundle.
