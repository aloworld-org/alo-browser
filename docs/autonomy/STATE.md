# Journal

One entry per loop iteration, newest last. What was built, what the gate said,
and anything the next iteration should know before it starts.

`LOOP COMPLETE` and `LOOP HALT` are read from this file by the supervisor, so
they appear only when they are true.

---

## 2026-09-02 — before the first iteration

Nothing is built. The repository holds its constitution, two decisions and a
queue.

What the first iteration should know:

- **Read `docs/decisions/0002` before touching layout.** It says the layout tree
  *is* the agent's tree, and that constrains the shape of the box tree from the
  first commit. Retrofitting it means rewriting layout.
- **Item 3 is not optional decoration.** `alo-workplace`'s design system is
  custom properties throughout, so an engine that cannot resolve `var()` renders
  nothing of alo at all — which is the whole target of stage 1.
- **Assert numbers, not images.** A layout test says where the box is. Reference
  renders exist as well, but a failure reading "row three moved 4px" is worth ten
  reading "the image differs".
- **The output is a PNG from a software rasteriser**, deliberately: it needs no
  GPU and no window, it is deterministic, and it makes every visual change
  reviewable as a diff. Hardware acceleration comes after correctness.
- **Sibling repositories worth reading, never editing:** `alo-os` for the shell
  that will embed this and for its verb contract, and `alo-workplace` for
  `web/src/ds/tokens.css` — which is the specification for what "correct colour"
  means here.
