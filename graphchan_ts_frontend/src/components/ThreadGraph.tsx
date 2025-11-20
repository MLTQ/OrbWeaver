import React, { useEffect, useRef } from 'react';
import * as d3 from 'd3';
import { type PostView } from '../api/types';

interface ThreadGraphProps {
    posts: PostView[];
    onNodeClick?: (postId: string) => void;
}

interface GraphNode extends d3.SimulationNodeDatum {
    id: string;
    group: number;
    post: PostView;
}

interface GraphLink extends d3.SimulationLinkDatum<GraphNode> {
    source: string | GraphNode;
    target: string | GraphNode;
}

export const ThreadGraph: React.FC<ThreadGraphProps> = ({ posts, onNodeClick }) => {
    const svgRef = useRef<SVGSVGElement>(null);

    useEffect(() => {
        if (!posts.length || !svgRef.current) return;

        const width = svgRef.current.clientWidth;
        const height = svgRef.current.clientHeight;

        // Clear previous graph
        d3.select(svgRef.current).selectAll('*').remove();

        const svg = d3.select(svgRef.current)
            .attr('viewBox', [0, 0, width, height])
            .style('font', '10px sans-serif');

        // Create nodes and links
        const nodes: GraphNode[] = posts.map(p => ({
            id: p.id,
            group: 1,
            post: p
        }));

        const links: GraphLink[] = [];
        posts.forEach(p => {
            p.parents.forEach(parentId => {
                // Only add link if parent exists in the current set of posts
                if (posts.find(parent => parent.id === parentId)) {
                    links.push({ source: parentId, target: p.id });
                }
            });
        });

        // Simulation setup
        const simulation = d3.forceSimulation(nodes)
            .force('link', d3.forceLink<GraphNode, GraphLink>(links).id(d => d.id).distance(100))
            .force('charge', d3.forceManyBody().strength(-300))
            .force('center', d3.forceCenter(width / 2, height / 2))
            .force('collide', d3.forceCollide().radius(30));

        // Draw elements
        const link = svg.append('g')
            .attr('stroke', '#333')
            .attr('stroke-opacity', 0.6)
            .selectAll('line')
            .data(links)
            .join('line')
            .attr('stroke-width', 1.5);

        const node = svg.append('g')
            .attr('stroke', '#fff')
            .attr('stroke-width', 1.5)
            .selectAll('circle')
            .data(nodes)
            .join('circle')
            .attr('r', 8)
            .attr('fill', d => d.post.files.length > 0 ? '#ff9900' : '#00f3ff')
            .call(drag(simulation) as any)
            .on('click', (_, d) => {
                if (onNodeClick) onNodeClick(d.id);
            });

        node.append('title')
            .text(d => `${d.id.substring(0, 8)}: ${d.post.body.substring(0, 50)}...`);

        // Labels
        const labels = svg.append('g')
            .selectAll('text')
            .data(nodes)
            .join('text')
            .attr('dx', 12)
            .attr('dy', 4)
            .text(d => d.id.substring(0, 4))
            .attr('fill', '#a0a0a0')
            .style('pointer-events', 'none');

        // Update positions
        simulation.on('tick', () => {
            link
                .attr('x1', d => (d.source as GraphNode).x!)
                .attr('y1', d => (d.source as GraphNode).y!)
                .attr('x2', d => (d.target as GraphNode).x!)
                .attr('y2', d => (d.target as GraphNode).y!);

            node
                .attr('cx', d => d.x!)
                .attr('cy', d => d.y!);

            labels
                .attr('x', d => d.x!)
                .attr('y', d => d.y!);
        });

        // Zoom support
        const zoom = d3.zoom<SVGSVGElement, unknown>()
            .scaleExtent([0.1, 4])
            .on('zoom', (event) => {
                svg.selectAll('g').attr('transform', event.transform);
            });

        svg.call(zoom);

        return () => {
            simulation.stop();
        };
    }, [posts, onNodeClick]);

    // Drag behavior
    const drag = (simulation: d3.Simulation<GraphNode, undefined>) => {
        function dragstarted(event: any) {
            if (!event.active) simulation.alphaTarget(0.3).restart();
            event.subject.fx = event.subject.x;
            event.subject.fy = event.subject.y;
        }

        function dragged(event: any) {
            event.subject.fx = event.x;
            event.subject.fy = event.y;
        }

        function dragended(event: any) {
            if (!event.active) simulation.alphaTarget(0);
            event.subject.fx = null;
            event.subject.fy = null;
        }

        return d3.drag()
            .on('start', dragstarted)
            .on('drag', dragged)
            .on('end', dragended);
    };

    return (
        <div style={{ width: '100%', height: '600px', background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
            <svg ref={svgRef} style={{ width: '100%', height: '100%' }} />
        </div>
    );
};
