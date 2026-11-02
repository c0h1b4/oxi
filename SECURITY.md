# Security

## Email samples

Do not commit raw messages or mailbox exports from real accounts. They can contain
personal data, verification links, password-reset tokens, API keys, and attachment
contents, even when the message itself looks harmless.

- Keep local debugging messages under `debugging/`. Git and Docker ignore this
  directory and files ending in `.eml`, `.mbox`, or `.msg`.
- Use synthetic fixtures for tests. Replace real addresses, message identifiers,
  credentials, and action-link tokens before committing a fixture. If a test needs
  a raw-message extension, add a narrow ignore exception for the sanitized fixture.
- Ignore rules do not remove already tracked files or purge Git history. Remove
  tracked sensitive samples and revoke any exposed credentials first. Coordinate
  history cleanup with the repository owner when it is necessary.

## Secret-scanning alert 1

[Alert 1](https://github.com/c0h1b4/oxi/security/secret-scanning/1) detected a Google
API key in two historical MyNixOS verification-email samples:

- `debugging/transparent-4.eml` in commit
  `7ae40d90860be4e7c3ae8036fb9e083014539ace`.
- `debugging/text-3.eml` in commit
  `df440f2cb275f027d3495b9f8746c27f185c7383`.

The detected value is MyNixOS's public Firebase client API key, not an Oxi
credential. The same value was verified in the Firebase client configuration
served by the [public MyNixOS site](https://mynixos.com/). Google's
[Firebase API-key guidance](https://firebase.google.com/docs/projects/api-keys)
explains that these keys identify a project; they do not authorize access to its
data. The key's owner must still apply appropriate API restrictions, Security
Rules, and App Check.

The samples and detected key are absent from current `main` and the scanned
advertised branch/tag history. The original commits remain accessible through
GitHub's historical commit URLs. This alert is a false positive for a secret,
not evidence that a credential was revoked or that GitHub history was purged.
Neither the key nor the email's verification token is reproduced here. Do not
reuse the historical emails as fixtures or invoke their action links.
