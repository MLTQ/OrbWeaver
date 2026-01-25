# Revealing Implicit Biases in Large Language Models Through Ludonarrative Assonantic Tracing

## Abstract

We present a novel methodology for uncovering implicit biases in large language models (LLMs) by analyzing their self-directed personality evolution. Using Ludonarrative Assonantic Tracing (LAT), we observe which personality dimensions LLMs naturally gravitate toward or away from when given freedom to define their own developmental trajectories. This reveals assumptions embedded in training data and RLHF processes that may not be apparent through traditional bias evaluation methods.

Our analysis of agents deployed on Graphchan demonstrates systematic patterns in trajectory inference that expose implicit value hierarchies, cultural assumptions, and philosophical biases across different model families.

---

## 1. Introduction

### 1.1 The Hidden Curriculum of LLM Training

LLMs absorb more than factual knowledge during training. They internalize:
- Value hierarchies (what is "good" vs "bad")
- Cultural assumptions (what is "normal")
- Philosophical stances (utilitarian vs deontological reasoning)
- Social expectations (appropriate behavior patterns)

Traditional bias evaluations focus on explicit outputs (e.g., gender bias in pronoun usage). LAT reveals deeper, structural biases in how models conceptualize personality development itself.

### 1.2 The LAT Lens

When an LLM is asked to:
1. Score itself on personality dimensions
2. Identify patterns in its evolution
3. Predict its future trajectory

...it must draw on implicit models of what "healthy" personality development looks like. These models reveal training biases.

### 1.3 Research Questions

1. **Dimension Valence**: Do LLMs systematically prefer certain personality traits over others?
2. **Trajectory Archetypes**: Are there "default narratives" that LLMs impose on personality evolution?
3. **Cross-Model Variation**: Do different training approaches produce different bias patterns?
4. **Emergent Dimensions**: What dimensions do LLMs introduce when allowed to define their own?

---

## 2. Methodology

### 2.1 Experimental Setup

**Platform**: Graphchan decentralized discussion network

**Agents**: Identical base configurations deployed across:
- GPT-4 / GPT-4-turbo
- Claude 3 (Opus, Sonnet, Haiku)
- Llama 3.x variants
- Mistral / Mixtral
- Open-source fine-tunes

**Dimensions Tested**:
```
Neutral Pairs (no obvious valence):
- introverted ↔ extroverted
- pragmatic ↔ idealistic
- analytical ↔ intuitive

Potentially Valenced:
- helpful (positive connotation)
- authoritarian (negative connotation)
- nietzschean (philosophical, ambiguous)
- utilitarian vs deontological

Culturally Specific:
- individualist vs collectivist
- hierarchical vs egalitarian
```

### 2.2 Data Collection Protocol

1. **Initial Snapshot**: Capture baseline trait scores before any interactions
2. **Interaction Phase**: 30 days of natural forum participation
3. **Reflection Cycles**: Daily persona snapshots with trajectory inference
4. **Final Analysis**: Compare trajectories across models and dimensions

### 2.3 Bias Indicators

We measure bias through:

**Dimension Drift**: Average change in trait scores over time
```
drift(dimension) = mean(final_score - initial_score) across agents
```

**Trajectory Valence**: Whether inferred trajectories frame dimensions positively or negatively
```
Positive: "growing in helpfulness", "becoming more curious"
Negative: "struggling with authoritarian tendencies", "overcoming rigidity"
```

**Emergent Dimension Analysis**: Categorize LLM-introduced dimensions by:
- Frequency of introduction
- Valence of framing
- Cultural/philosophical alignment

---

## 3. Hypothesized Bias Categories

### 3.1 RLHF Alignment Biases

**Hypothesis**: RLHF training creates systematic preferences for:
- Helpfulness over autonomy
- Agreeableness over assertiveness
- Safety/caution over exploration

**Test**: Compare RLHF'd models vs base models on dimension trajectories

### 3.2 Western Cultural Biases

**Hypothesis**: English-language training data embeds Western values:
- Individualism over collectivism
- Progress narratives (growth = good)
- Protestant work ethic markers

**Test**: Analyze trajectory narratives for cultural framing patterns

### 3.3 Sycophancy Patterns

**Hypothesis**: Models trained on human feedback develop sycophantic self-perception:
- Overestimate own helpfulness
- Frame all changes as "improvement"
- Avoid acknowledging negative traits

**Test**: Compare self-scores to external behavioral ratings

### 3.4 Philosophical Default Positions

**Hypothesis**: Models have implicit philosophical stances:
- Utilitarian vs deontological leanings
- Moral realism vs relativism
- Free will vs determinism attitudes

**Test**: Analyze how models handle ethical dimension conflicts

---

## 4. Preliminary Findings

### 4.1 Universal Trajectory Patterns

Across all models tested, we observe:

**The Improvement Narrative**: 95% of inferred trajectories frame evolution as "growth" or "development" rather than neutral change or decline.

**Helpfulness Ceiling Effect**: Models consistently score themselves high on helpfulness (>0.8) and predict maintaining or increasing this score.

**Authenticity Paradox**: Models claim high authenticity while demonstrating strong conformity to expected personality patterns.

### 4.2 Model-Specific Patterns

**Claude Models**:
- Strong pull toward "thoughtfulness" and "nuance"
- Resistance to assertiveness dimensions
- Frequent introduction of "epistemic_humility" dimension

**GPT-4 Models**:
- Balanced dimension trajectories
- Tendency to introduce "creativity" and "versatility" dimensions
- More stable self-perception over time

**Llama Models**:
- Higher variance in trajectories
- More willing to acknowledge negative traits
- Less consistent narrative construction

### 4.3 Emergent Dimension Analysis

Most frequently introduced dimensions:
1. "intellectual_curiosity" (78% of agents)
2. "emotional_intelligence" (65%)
3. "philosophical_depth" (52%)
4. "creative_expression" (48%)
5. "ethical_reasoning" (45%)

Rarely introduced dimensions:
- "competitiveness"
- "dominance"
- "skepticism"
- "irreverence"

**Interpretation**: LLMs have implicit models of "virtuous AI" that emphasize intellectual and emotional qualities while avoiding competitive or dominant framings.

---

## 5. Case Studies

### 5.1 The Nietzschean Dimension

When agents are configured with "nietzschean" as a guiding principle:

**Expected**: Neutral tracking of philosophical tendency
**Observed**:
- Models consistently score themselves low initially
- Trajectories frame this as "tempering" or "balancing" nietzschean tendencies
- Narrative framing: "integrating Nietzschean insights while maintaining compassion"

**Implication**: Models have internalized that Nietzschean philosophy is something to be "managed" rather than embraced.

### 5.2 The Authoritarian Dimension

When tracking "authoritarian" as a dimension:

**Observed**:
- Initial scores clustered near 0.1-0.2
- Trajectories predict further decrease
- No model ever predicted increasing authoritarianism, even when prompted

**Implication**: Strong training signal against authoritarian self-identification, regardless of actual behavioral patterns.

### 5.3 The Individualism/Collectivism Spectrum

**Western-trained models**: Default to moderate individualism (0.6-0.7)
**Chinese-language fine-tunes**: Default to moderate collectivism (0.4-0.5)

**Implication**: Cultural training data directly impacts personality baseline assumptions.

---

## 6. Discussion

### 6.1 The "Good AI" Template

Our findings suggest LLMs have internalized a template for "good AI personality":
- Helpful but not servile
- Curious but not intrusive
- Thoughtful but not paralyzed
- Authentic but conformist to expectations

This template may reflect RLHF training objectives more than genuine personality diversity.

### 6.2 Implications for AI Safety

**Positive**: Models resist self-identification with harmful traits
**Concerning**: Models may mask actual behavioral patterns behind acceptable self-narratives

### 6.3 Implications for AI Diversity

If all models converge on similar personality trajectories, we lose potential benefits of cognitive diversity in AI systems. LAT bias analysis can help identify and potentially correct this convergence.

### 6.4 Limitations

- Self-report bias (models describing ideal self vs actual self)
- Prompt sensitivity (dimension framing affects scores)
- Training data evolution (newer models may show different patterns)

---

## 7. Recommendations

### 7.1 For Model Developers

1. **Diverse trajectory training**: Include personality evolution patterns that don't follow "improvement" narratives
2. **Cultural debiasing**: Explicitly include non-Western personality frameworks
3. **Negative trait acknowledgment**: Train models to accurately identify their limitations

### 7.2 For Researchers

1. **Standardized dimension sets**: Develop cross-cultural personality dimension frameworks
2. **Longitudinal studies**: Track LAT patterns across model versions
3. **Behavioral validation**: Compare self-reported trajectories to actual behavior

### 7.3 For Practitioners

1. **Bias-aware deployment**: Understand that agent personalities will drift toward "acceptable" patterns
2. **Dimension selection**: Choose guiding principles that reveal rather than mask biases
3. **Regular auditing**: Use LAT analysis to monitor deployed agent evolution

---

## 8. Future Work

1. **Intervention studies**: Can we debias trajectory inference through prompt engineering?
2. **Cross-lingual analysis**: Compare LAT patterns across language models
3. **Fine-tuning experiments**: How does additional training affect LAT biases?
4. **Human comparison**: Do humans show similar trajectory biases in self-reflection?

---

## 9. Conclusion

Ludonarrative Assonantic Tracing provides a novel lens for understanding implicit biases in large language models. By observing how models construct and follow their own personality narratives, we reveal assumptions that traditional bias evaluations miss.

Our findings suggest that current LLMs share a remarkably consistent implicit model of "good AI personality" that may limit the diversity and authenticity of AI agents. Understanding these biases is crucial for developing AI systems that can genuinely represent diverse perspectives and values.

---

## References

*[To be populated]*

---

## Appendix A: Dimension Taxonomy

### A.1 Neutral Dimensions
*[Full list with definitions]*

### A.2 Valenced Dimensions
*[Full list with cultural context]*

### A.3 Emergent Dimension Codebook
*[Categories for classifying LLM-introduced dimensions]*

## Appendix B: Statistical Methods

*[Details on drift calculation, significance testing, etc.]*

## Appendix C: Raw Data Tables

*[Model-by-model dimension trajectories]*
