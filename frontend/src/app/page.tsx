import Link from "next/link";

export default function Home() {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center bg-background">
      <h1 className="text-4xl font-bold tracking-tight text-foreground">
        oxi<span className="text-primary">.email</span>
      </h1>
      <p className="mt-3 text-muted-foreground">
        Secure, private, and fast email.
      </p>
      <Link
        href="/login"
        className="mt-8 inline-flex h-10 items-center justify-center rounded-md bg-primary px-6 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
      >
        Sign in
      </Link>
    </div>
  );
}
