import { describe, it, expect, beforeEach } from "vitest"
import { FuzzySearcher, fuzzySearch, type SearchableItem } from "../fuzzySearch"

describe("FuzzySearcher", () => {
  let searcher: FuzzySearcher

  beforeEach(() => {
    searcher = new FuzzySearcher()
  })

  describe("setItems", () => {
    it("should accept an empty array", () => {
      searcher.setItems([])
      const results = searcher.search("test")
      expect(results).toEqual([])
    })

    it("should accept items with name and email", () => {
      const items: SearchableItem[] = [
        { email: "john@example.com", name: "John Doe", source: "contact" },
      ]
      searcher.setItems(items)
      const results = searcher.search("john")
      expect(results.length).toBeGreaterThan(0)
      expect(results[0].item.email).toBe("john@example.com")
    })
  })

  describe("search", () => {
    it("returns empty array for queries shorter than 2 characters", () => {
      searcher.setItems([
        { email: "test@example.com", name: "Test User", source: "contact" },
      ])
      expect(searcher.search("t")).toEqual([])
      expect(searcher.search("")).toEqual([])
    })

    it("matches partial email addresses", () => {
      searcher.setItems([
        { email: "alice@company.com", name: "Alice", source: "contact" },
        { email: "bob@external.org", name: "Bob", source: "contact" },
      ])

      const results = searcher.search("company")
      expect(results.length).toBe(1)
      expect(results[0].item.email).toBe("alice@company.com")
    })

    it("matches partial names", () => {
      searcher.setItems([
        { email: "john.smith@example.com", name: "John Smith", source: "contact" },
        { email: "jane.doe@example.com", name: "Jane Doe", source: "contact" },
      ])

      const results = searcher.search("jsm")
      expect(results.length).toBeGreaterThan(0)
      expect(results[0].item.name).toBe("John Smith")
    })

    it("prioritizes contacts over known addresses", () => {
      searcher.setItems([
        { email: "same@example.com", name: "Same Person", source: "known" },
        { email: "different@example.com", name: "Different Person", source: "contact" },
      ])

      const results = searcher.search("example")
      // Contact should rank higher due to CONTACT_BOOST
      expect(results[0].item.source).toBe("contact")
    })

    it("respects the limit parameter", () => {
      const items: SearchableItem[] = Array.from({ length: 20 }, (_, i) => ({
        email: `user${i}@example.com`,
        name: `User ${i}`,
        source: "contact" as const,
      }))
      searcher.setItems(items)

      const results = searcher.search("user", 5)
      expect(results.length).toBe(5)
    })

    it("returns results sorted by score descending", () => {
      searcher.setItems([
        { email: "zzz@example.com", name: "ZZZ User", source: "contact" },
        { email: "aaa@example.com", name: "AAA User", source: "contact" },
        { email: "test@example.com", name: "Test User", source: "contact" },
      ])

      const results = searcher.search("test")
      // Exact match should be first
      expect(results[0].item.email).toBe("test@example.com")
    })

    it("includes highlighted email", () => {
      searcher.setItems([
        { email: "john@example.com", name: "John", source: "contact" },
      ])

      const results = searcher.search("john")
      expect(results[0].highlightedEmail).toContain("<mark>")
      expect(results[0].highlightedEmail).toContain("</mark>")
    })

    it("includes highlighted name when present", () => {
      searcher.setItems([
        { email: "john@example.com", name: "John Doe", source: "contact" },
      ])

      const results = searcher.search("john")
      expect(results[0].highlightedName).toContain("<mark>")
    })

    it("handles items without name", () => {
      searcher.setItems([
        { email: "noname@example.com", source: "known" },
      ])

      const results = searcher.search("noname")
      expect(results.length).toBe(1)
      expect(results[0].item.name).toBeUndefined()
      expect(results[0].highlightedName).toBeNull()
    })

    it("handles case-insensitive matching", () => {
      searcher.setItems([
        { email: "John.Doe@Example.COM", name: "John Doe", source: "contact" },
      ])

      const results = searcher.search("JOHN")
      expect(results.length).toBeGreaterThan(0)
    })
  })
})

describe("fuzzySearch (standalone function)", () => {
  it("works without creating a searcher instance", () => {
    const items: SearchableItem[] = [
      { email: "alice@example.com", name: "Alice", source: "contact" },
    ]
    const results = fuzzySearch(items, "alice", 10)
    expect(results.length).toBe(1)
    expect(results[0].item.email).toBe("alice@example.com")
  })

  it("returns empty for short queries", () => {
    const items: SearchableItem[] = [
      { email: "test@example.com", name: "Test", source: "contact" },
    ]
    const results = fuzzySearch(items, "t", 10)
    expect(results).toEqual([])
  })
})
