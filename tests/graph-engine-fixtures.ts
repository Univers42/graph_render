/* ************************************************************************** */
/*                                                                            */
/*                                                        :::      ::::::::   */
/*   graph-engine-fixtures.ts                           :+:      :+:    :+:   */
/*                                                    +:+ +:+         +:+     */
/*   By: dlesieur <dlesieur@student.42.fr>          +#+  +:+       +#+        */
/*                                                +#+#+#+#+#+   +#+           */
/*   Created: 2026/06/08 12:00:00 by dlesieur          #+#    #+#             */
/*   Updated: 2026/06/08 12:00:00 by dlesieur         ###   ########.fr       */
/*                                                                            */
/* ************************************************************************** */

import type { GraphEdge, GraphNode } from "../src/core/types.ts";

export function node(id: string, kind: GraphNode["kind"], databaseId: string | null, weight = 0): GraphNode {
  return { id, kind, databaseId, source: "db", label: id, group: null, weight, version: 0, hasNote: false };
}
export function edge(id: string, source: string, target: string, kind: GraphEdge["kind"], strength = 1): GraphEdge {
  return { id, source, target, kind, label: kind, strength, directed: true };
}
