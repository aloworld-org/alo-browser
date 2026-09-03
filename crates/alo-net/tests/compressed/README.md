# Frozen compressed bodies

Bytes, as some other implementation produced them. That is the point: a suite
that compressed with the same crate it decompresses with proves the crate is
self-consistent, which is not the question. The question is whether we can read
what a server sent, and every one of these was made by something that is not us.

`page.html` is the plaintext all of them decode back to.

Re-derive any of them:

```sh
gzip -9 -n -c page.html > page.html.gz
brotli -q 11 -c page.html > page.html.br
zstd  -19 -q -c page.html > page.html.zst
python3 -c "import zlib,pathlib
raw = pathlib.Path('page.html').read_bytes()
pathlib.Path('page.html.zz').write_bytes(zlib.compress(raw, 9))
c = zlib.compressobj(9, zlib.DEFLATED, -15)
pathlib.Path('page.html.deflate').write_bytes(c.compress(raw) + c.flush())"
```

`page.html.zz` is zlib-wrapped DEFLATE, which is what `Content-Encoding:
deflate` is specified to mean. `page.html.deflate` is raw DEFLATE with no
wrapper, which is what a meaningful number of servers send under that same
name. Both are here because both have to work.

`bomb.gz` is eight kibibytes that decode to eight mebibytes — a thousand to
one, and nothing about gzip stops it being a million to one:

```sh
python3 -c "import zlib,pathlib
z = zlib.compressobj(9, zlib.DEFLATED, 31)
pathlib.Path('bomb.gz').write_bytes(z.compress(b'\0' * (8*1024*1024)) + z.flush())"
```

It is not malware and it is not anybody's page. It is the shape of the attack,
small enough to keep in a repository and run in a test that finishes.
