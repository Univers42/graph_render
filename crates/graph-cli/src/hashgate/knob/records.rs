//! The record each negative control's run writes, one arm per [`super::Knob`].
//!
//! Split out of `knob.rs` by the house's 300-line limit, and because the record name is
//! the one thing about a control that is pure bookkeeping: it names where the run leaves
//! its evidence, nothing more. `Knob::record` is the only caller and it delegates here
//! rather than keeping a second list, so a control cannot write one record and be counted
//! under another.
//!
//! The six igraph layout controls read [`super::igraph::RECORD`] by their own arm index
//! rather than spelling their names out, for the reason `igraph::ENV` gives.

use super::Knob;
use super::igraph;
use super::knobs;
use super::three_d;

/// The record `knob`'s run writes.
pub const fn record(knob: Knob) -> &'static str {
    match knob {
        Knob::ReferenceDegree => "hashgate-control-reference-degree",
        Knob::GridSpacing => "hashgate-control-grid-spacing",
        Knob::SugiyamaLayerSpacing => "hashgate-control-sugiyama-layer-spacing",
        Knob::NodeCount => "hashgate-control-node-count",
        Knob::ForceTheta => "hashgate-control-force-theta",
        Knob::Fa2ScalingRatio => "hashgate-control-fa2-scaling-ratio",
        Knob::TreeTidyNodes => "hashgate-control-tree-tidy-nodes",
        Knob::TreemapNodes => "hashgate-control-treemap-nodes",
        Knob::CircularNodes => "hashgate-control-circular-nodes",
        Knob::SpringIterations => "hashgate-control-spring-iterations",
        Knob::CircularHierarchyNodes => "hashgate-control-circular-hierarchy-nodes",
        Knob::PackingScale => "hashgate-control-packing-scale",
        Knob::AnalysisComponentsWeak => "hashgate-control-analysis-components-weak",
        Knob::AnalysisComponentsStrong => "hashgate-control-analysis-components-strong",
        Knob::AnalysisCommunitiesLouvain => "hashgate-control-analysis-communities-louvain",
        Knob::AnalysisCentralityDegree => "hashgate-control-analysis-centrality-degree",
        Knob::AnalysisCentralityCloseness => "hashgate-control-analysis-centrality-closeness",
        Knob::AnalysisCentralityBetweenness => "hashgate-control-analysis-centrality-betweenness",
        Knob::AnalysisCentralityEigenvector => "hashgate-control-analysis-centrality-eigenvector",
        Knob::AnalysisDepthBfs => "hashgate-control-analysis-depth-bfs",
        Knob::PostBundleFdeb => "hashgate-control-post-bundle-fdeb",
        Knob::PostBundleMingle => "hashgate-control-post-bundle-mingle",
        Knob::PostRouteGrid => "hashgate-control-post-route-grid",
        Knob::PostStyleStraight => "hashgate-control-post-style-straight",
        Knob::PostStyleOrthogonal => "hashgate-control-post-style-orthogonal",
        Knob::PostStyleQuadratic => "hashgate-control-post-style-quadratic",
        Knob::PostStyleBezier => "hashgate-control-post-style-bezier",
        Knob::IgraphFruchtermanReingoldNodes => igraph::RECORD[0],
        Knob::IgraphKamadaKawaiNodes => igraph::RECORD[1],
        Knob::IgraphGraphoptNodes => igraph::RECORD[2],
        Knob::IgraphDavidsonHarelNodes => igraph::RECORD[3],
        Knob::IgraphLglNodes => igraph::RECORD[4],
        Knob::IgraphDrlNodes => igraph::RECORD[5],
        Knob::Basic3dSphereNodes => three_d::RECORD[0],
        Knob::Basic3dHelixNodes => three_d::RECORD[1],
        Knob::Basic3dCubeNodes => three_d::RECORD[2],
        Knob::Hierarchical3dNodes => three_d::RECORD[3],
        Knob::Spring3dNodes => three_d::RECORD[4],
        Knob::Basic3dSpiralNodes => three_d::RECORD[5],
        Knob::Bipartite3dNodes => three_d::RECORD[6],
        Knob::PackingOsageNodes => knobs::OSAGE_LAYOUT_STAGES[0].record,
        Knob::TwopiNodes => "hashgate-control-twopi-nodes",
        Knob::NeatoEpsilon => "hashgate-control-neato-epsilon",
        Knob::PatchworkNodes => "hashgate-control-patchwork-nodes",
        Knob::SplitSum => "hashgate-control-split-sum",
        Knob::SplitRescale => "hashgate-control-split-rescale",
        Knob::LayoutParamDefault => "hashgate-control-layout-param-default",
        Knob::OverlapRelaxation => "hashgate-control-overlap-relaxation",
        Knob::ForceSessionGravity => "forcegate-control-force-session-gravity",
    }
}
