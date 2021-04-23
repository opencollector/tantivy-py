import pathlib
import hashlib
import html
from urllib.parse import quote

TEMPLATE = """
<!DOCTYPE html>
<html>
  <head>
    <meta name="pypi:repository-version" content="1.0">
    <title>Links for tantivy_oc_fork</title>
  </head>
</html>
<body>
  <h1>Links for tantivy_oc_fork</h1>
  {links}
</body>
</html>
"""

DOWNLOAD_BASE_URL_TEMPLATE = "https://github.com/opencollector/tantivy-py/releases/download/{release}/{file}#sha256={digest}"
LINK_TEMPLATE = """<a href="{url}">{name}</a>"""

RELEASES = {
    "oc-v0.16.0-dev0": "0.16.0.dev0",
    "oc-v0.22.0-dev0": "0.22.0.dev0",
    "oc-v0.14.0": "0.14.0",
}

print(
    TEMPLATE.format(
        links="\n  ".join(
            LINK_TEMPLATE.format(
                url=html.escape(
                    DOWNLOAD_BASE_URL_TEMPLATE.format(
                        release=quote(release),
                        file=quote(f.name),
                        digest=quote(hashlib.sha256(f.read_bytes()).hexdigest()),
                    ),
                ),
                name=html.escape(f.name),
            )
            for release, f in (
                ([k for k, v in RELEASES.items() if v in f.name][0], f)
                for f in sorted(pathlib.Path("target/wheels").glob("*.whl"))
            )
        )
    )
)
