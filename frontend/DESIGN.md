# bird design language

Black and white, square, quiet. The dashboard is a tool for people who run servers: it should feel
precise and fast, never decorative. Color means something or it isn't there.

## Color

Only the tokens in `src/index.css` exist. Tailwind's palette is turned off (`--color-*: initial`),
so a class like `bg-gray-800` produces nothing; that is on purpose.

| Token                        | Use                                                       |
| ---------------------------- | --------------------------------------------------------- |
| `sunken`                     | Inputs, logs, terminals, code: things you look _into_     |
| `canvas`                     | The page                                                  |
| `surface`                    | Panels, lists, cards, dialogs                             |
| `raised`                     | Hover, selected, pressed                                  |
| `line`, `line-strong`        | Hairline borders; strong for emphasis and floating things |
| `fg`, `fg-muted`, `fg-faint` | Text: primary, secondary, tertiary                        |
| `ok`, `warn`, `bad`, `info`  | Status only, and always next to a word. Never color alone |

All greys have zero chroma. Status colors are the only hue in the interface.

## Shape and depth

- Corners are square. Small controls get at most 2px.
- Boxes are 1px hairlines on `surface`. There are no shadows: on black they read as smudges, not
  depth.
- Things that float (menus, dialogs, toasts) are lifted by a stronger border, `border-line-strong`.
- The header is a `material`: translucent with a blur, content scrolls under it. It turns solid
  under `prefers-reduced-transparency`.

## Type

- Geist for the interface, Geist Mono for anything that is data: ids, images, commands, times,
  counts, and the small uppercase labels.
- Sizes are rem tokens so the layout follows the browser's text size: `text-2xs` (11px labels),
  `text-ui` (13px dense UI), `text-sm` (body), `text-title` (dialog titles), `text-2xl` (page titles).
- Tracking depends on size: headings tighten (`h1` -0.02em, `h2`/`h3` -0.01em), body stays at 0,
  mono labels open up to 0.08em.
- Leading tightens as type grows: 1.1 for page titles, 1.5 for body.
- Hierarchy comes from weight and size together; emphasis is weight, not color.

The small uppercase mono label (`Eyebrow`, `eyebrowClass`) titles sections, table columns and
metadata. It is the most recognizable piece of the language; use it instead of inventing headings.

## Motion

- Feedback starts on press: interactive things use `pressable`, which scales to 0.97 on `:active`.
- Animations are springs from `src/lib/motion.ts`, critically damped (no overshoot). Nothing in
  this app is thrown, so nothing bounces.
- Only `transform` and `opacity` animate.
- Things leave the way they came: toasts slide back out to the right, menus shrink toward their
  trigger, the sign-in steps slide back when you go back.
- `prefers-reduced-motion` reduces every transition to a near-instant change; Motion follows the
  same setting through `MotionConfig reducedMotion="user"`.

## Components

Pages are assembled from `src/components/ui`; a page writing its own border, focus ring or input
classes is a sign a primitive is missing.

| Need                                      | Use                                                          |
| ----------------------------------------- | ------------------------------------------------------------ |
| A box                                     | `Panel` (`tone="danger"` or `"notice"` for warnings)         |
| Rows of things                            | `List`, `ListItem`, `ListText`                               |
| Page and section titles                   | `PageHeader` (with `eyebrow`), `Section`                     |
| Icon-only action                          | `IconButton` with a `label`                                  |
| One of a few choices that need explaining | `ChoiceGroup` and `ChoiceCard`                               |
| Destructive action                        | `ConfirmDialog`, with `confirmText` when it cannot be undone |
| Irreversible, page-level                  | `DangerZone`                                                 |
| Something shown once                      | `SecretBox`                                                  |
| Status                                    | `Badge` with a `tone` and a word                             |
| Nothing yet / failed to load              | `EmptyState`, `ErrorState`                                   |

Ask for confirmation only before something destructive. Safe actions, like a redeploy that only
moves traffic once the new machines are healthy, run at once and report with a toast.

## Forms

- TanStack Form with Zod. `useAppForm` (`src/components/form/app-form.ts`) comes with our fields
  pre-bound: `field.TextField`, `PasswordField`, `TextareaField`, `NumberField`, `SelectField`,
  `CheckboxField`, `CodeField`.
- Rules live in Zod schemas: shared ones in `src/lib/validation.ts` mirror birdd's own checks, and
  feature-specific ones sit next to the form or in the feature's `schemas.ts`.
- Errors appear once a field has been left or the form was sent, then update as you type.
- Forms in dialogs use `useDialogForm` and `FormDialog`: the dialog closes when the submit
  resolves and stays open showing birdd's error, with the app's log lines when a deploy failed.

## Accessibility

- Every control has a visible label, or an `sr-only` one when a row would repeat it.
- Focus is a 1px `fg` outline with a 2px offset (`focusRing`).
- Errors are linked with `aria-describedby` and alerts use `role="alert"`.
- Status never relies on color alone.
- `prefers-contrast: more` strengthens lines and secondary text.

## The mark

The pixel-art bird in `assets/logo.png` is the logo. `frontend/public` and `src/assets` hold
pre-scaled copies (box-filtered so the pixels stay crisp); regenerate them from the source rather
than scaling the big file in the browser.
