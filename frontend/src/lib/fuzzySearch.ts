import fuzzysort from "fuzzysort"

export interface SearchableItem {
  email: string
  name?: string
  source: "contact" | "known"
}

export interface SearchResult {
  item: SearchableItem
  score: number
  highlightedName: string | null
  highlightedEmail: string
}

interface PreparedItem {
  original: SearchableItem
  namePrepared: Fuzzysort.Prepared | null
  emailPrepared: Fuzzysort.Prepared
}

const MIN_QUERY_LENGTH = 2
const NAME_WEIGHT = 1.5
const CONTACT_BOOST = 100

export class FuzzySearcher {
  private preparedItems: PreparedItem[] = []

  setItems(items: SearchableItem[]): void {
    this.preparedItems = items.map((item) => ({
      original: item,
      namePrepared: item.name ? fuzzysort.prepare(item.name) : null,
      emailPrepared: fuzzysort.prepare(item.email),
    }))
  }

  search(query: string, limit: number = 10): SearchResult[] {
    if (query.length < MIN_QUERY_LENGTH) {
      return []
    }

    const results: SearchResult[] = []

    for (const prepared of this.preparedItems) {
      const nameResult = prepared.namePrepared
        ? fuzzysort.single(query, prepared.namePrepared)
        : null
      const emailResult = fuzzysort.single(query, prepared.emailPrepared)

      if (!nameResult && !emailResult) {
        continue
      }

      let score = 0

      if (nameResult) {
        score += nameResult.score * NAME_WEIGHT
      }

      if (emailResult) {
        score += emailResult.score
      }

      if (prepared.original.source === "contact") {
        score += CONTACT_BOOST
      }

      results.push({
        item: prepared.original,
        score,
        highlightedName: nameResult
          ? this.highlightMatch(nameResult)
          : null,
        highlightedEmail: emailResult
          ? this.highlightMatch(emailResult)
          : prepared.original.email,
      })
    }

    results.sort((a, b) => b.score - a.score)

    return results.slice(0, limit)
  }

  private highlightMatch(result: Fuzzysort.Result): string {
    return result.highlight("<mark>", "</mark>") ?? ""
  }
}

let searcherInstance: FuzzySearcher | null = null

export function fuzzySearch(
  items: SearchableItem[],
  query: string,
  limit: number = 10
): SearchResult[] {
  if (!searcherInstance) {
    searcherInstance = new FuzzySearcher()
  }
  searcherInstance.setItems(items)
  return searcherInstance.search(query, limit)
}
