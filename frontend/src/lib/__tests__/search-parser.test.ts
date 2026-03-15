import { describe, it, expect } from "vitest";
import { normalizeSearchQuery, isValidCommittedSearch } from "../search-parser";

describe("search-parser utilities", () => {
  describe("normalizeSearchQuery", () => {
    it("should trim leading and trailing whitespace", () => {
      expect(normalizeSearchQuery("  hello  ")).toBe("hello");
    });

    it("should collapse multiple internal spaces", () => {
      expect(normalizeSearchQuery("hello   world")).toBe("hello world");
    });

    it("should handle tabs and other whitespace", () => {
      expect(normalizeSearchQuery("hello\tworld \n test")).toBe("hello world test");
    });

    it("should return empty string for whitespace-only input", () => {
      expect(normalizeSearchQuery("   ")).toBe("");
      expect(normalizeSearchQuery("\t\t")).toBe("");
    });
  });

  describe("isValidCommittedSearch", () => {
    it("should reject empty strings as invalid", () => {
      expect(isValidCommittedSearch("")).toBe(false);
    });

    it("should reject whitespace-only strings as invalid", () => {
      expect(isValidCommittedSearch("   ")).toBe(false);
      expect(isValidCommittedSearch("\t")).toBe(false);
    });

    it("should reject single character queries as invalid", () => {
      expect(isValidCommittedSearch("a")).toBe(false);
      expect(isValidCommittedSearch(" a ")).toBe(false);
    });

    it("should accept queries with 2 or more characters", () => {
      expect(isValidCommittedSearch("ab")).toBe(true);
      expect(isValidCommittedSearch(" abc ")).toBe(true);
    });

    it("should accept operator-based queries", () => {
      expect(isValidCommittedSearch("has:attachment")).toBe(true);
      expect(isValidCommittedSearch("from:me")).toBe(true);
    });

    it("should accept queries that are valid after normalization", () => {
      expect(isValidCommittedSearch(" a b ")).toBe(true); 
    });
  });
});
