# Python basic future wrapper

Python is not the core. A future wrapper can use `pyo3`/`maturin` or `ctypes` over the C ABI.

```python
from pontemesh_sdk import PontemeshClient

client = PontemeshClient(
    origin_url="https://origin.example.com",
    application_token="application-token",
)

client.sync_object(
    bucket="assets",
    key="models/tree.glb",
    destination="./assets/models/tree.glb",
)
```

