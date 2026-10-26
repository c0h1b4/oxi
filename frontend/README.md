This is a [Next.js](https://nextjs.org) project bootstrapped with [`create-next-app`](https://nextjs.org/docs/app/api-reference/cli/create-next-app).

## Getting Started

First, run the development server:

```bash
npm run dev
# or
yarn dev
# or
pnpm dev
# or
bun dev
```

Open [http://localhost:3000](http://localhost:3000) with your browser to see the result.

You can start editing the page by modifying `app/page.tsx`. The page auto-updates as you edit the file.

This project uses [`next/font`](https://nextjs.org/docs/app/building-your-application/optimizing/fonts) to automatically optimize and load [Geist](https://vercel.com/font), a new font family for Vercel.

## Dependency updates and validation

Use Bun for dependency changes and Node.js 22.22.2+, 24.15.0+, or 26+ for
Node-based tools (matching the supported engine ranges in `package.json`).
From this directory, run:

```bash
bun update --latest
bun run lint
bun run test
bun run typecheck
bun run build
```

`@typescript/native` aliases the latest stable TypeScript 7 compiler and provides
`tsc`. The `typescript` dependency aliases Microsoft's `@typescript/typescript6`
API package because typescript-eslint still requires the TypeScript 6
JavaScript API. Keep both aliases until the lint tooling supports TypeScript 7.
This follows [Microsoft's side-by-side setup](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/#running-side-by-side-with-typescript-6.0).

The ProseMirror overrides keep `model`, `transform`, and `view` on a single
version across Tiptap's dependency graph. Multiple copies can break Enter and
list splitting because their node/fragment classes are not interchangeable.
Update these overrides together, regenerate the lockfile, and run the real
`RichTextEditor` keyboard regression tests when upgrading Tiptap/ProseMirror.
After changing overrides, remove `node_modules` before reinstalling: Bun may
otherwise retain stale nested copies even with `--frozen-lockfile`.

Commit `package.json`, `bun.lock`, and `.bun.nix` together. From the repository
root, regenerate the Nix snapshot with the pinned bun2nix input:

```bash
nix run github:baileyluTCD/bun2nix/c843f477b15f51151f8c6bcc886954699440a6e1 -- \
  --lock-file frontend/bun.lock --output-file frontend/.bun.nix
```

## Learn More

To learn more about Next.js, take a look at the following resources:

- [Next.js Documentation](https://nextjs.org/docs) - learn about Next.js features and API.
- [Learn Next.js](https://nextjs.org/learn) - an interactive Next.js tutorial.

You can check out [the Next.js GitHub repository](https://github.com/vercel/next.js) - your feedback and contributions are welcome!

## Deploy on Vercel

The easiest way to deploy your Next.js app is to use the [Vercel Platform](https://vercel.com/new?utm_medium=default-template&filter=next.js&utm_source=create-next-app&utm_campaign=create-next-app-readme) from the creators of Next.js.

Check out our [Next.js deployment documentation](https://nextjs.org/docs/app/building-your-application/deploying) for more details.
