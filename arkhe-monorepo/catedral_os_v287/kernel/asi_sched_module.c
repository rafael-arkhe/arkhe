// SPDX-License-Identifier: GPL-2.0
/*
 * asi_sched_module.c — modulo LKM de demonstracao
 * Catedral OS v287.0, BLOCO 829
 *
 * Consome os dois hooks do patch:
 *   android_rvh_asi_select_task_rq   (placement topologico, I298/I299)
 *   android_rvh_asi_task_should_scx  (guard de coexistencia, I296/I298)
 *
 * KMI-intacto: nada em `struct rq` nem em `struct task_struct`. A coerencia
 * phi (I296, dissipacao de Lindblad) e LOGICA DO MODULO — registro hash
 * pid -> phi escalado em [0..10000]. O kernel nunca a conhece.
 *
 * Contratos:
 *   - handler void (comunicacao por estado, sem retorno);
 *   - ponteiros nao reescritos => fallback estrutural (I301);
 *   - estatisticas per-cpu no modulo (I299).
 */

#include <linux/fs.h>
#include <linux/hash.h>
#include <linux/hashtable.h>
#include <linux/kernel.h>
#include <linux/module.h>
#include <linux/moduleparam.h>
#include <linux/percpu.h>
#include <linux/proc_fs.h>
#include <linux/sched.h>
#include <linux/sched/topology.h>
#include <linux/seq_file.h>
#include <linux/slab.h>
#include <linux/spinlock.h>
#include <linux/tracepoint.h>

#include <trace/hooks/sched.h>

#define ASI_PHI_SCALE 10000U

static unsigned int asi_phi_threshold = 9000;	/* I298: limiar topologico */
module_param_named(threshold, asi_phi_threshold, uint, 0644);

static bool asi_hook_enabled = true;		/* I296: gate Lindblad */
module_param_named(enabled, asi_hook_enabled, bool, 0644);

/* ------------------------------------------------------------------ */
/* Registro interno pid -> phi (deteccao de anomalias, I300).          */

struct asi_task {
	pid_t pid;
	unsigned int phi;
	struct hlist_node node;
};

static DEFINE_HASHTABLE(asi_tasks, 8);
static DEFINE_SPINLOCK(asi_lock);

/* API do modulo: o agente user-space (ou o orquestrador v287.0) registra
 * tarefas anomalas via `echo <pid> <phi> > /proc/asi_sched_tasks`. */
static void asi_task_register(pid_t pid, unsigned int phi)
{
	struct asi_task *e;
	unsigned long flags;

	spin_lock_irqsave(&asi_lock, flags);
	hash_for_each_possible(asi_tasks, e, node, pid) {
		if (e->pid == pid) {
			e->phi = phi;
			spin_unlock_irqrestore(&asi_lock, flags);
			return;
		}
	}
	e = kzalloc(sizeof(*e), GFP_ATOMIC);
	if (!e) {
		spin_unlock_irqrestore(&asi_lock, flags);
		return;
	}
	e->pid = pid;
	e->phi = min(phi, ASI_PHI_SCALE);
	hash_add(asi_tasks, &e->node, e->pid);
	spin_unlock_irqrestore(&asi_lock, flags);
}

static unsigned int asi_phi_of(struct task_struct *p)
{
	struct asi_task *e;
	unsigned long flags;
	unsigned int phi = 0;

	spin_lock_irqsave(&asi_lock, flags);
	hash_for_each_possible(asi_tasks, e, node, p->pid) {
		if (e->pid == p->pid) {
			phi = e->phi;
			break;
		}
	}
	spin_unlock_irqrestore(&asi_lock, flags);
	return phi;
}

/* ------------------------------------------------------------------ */
/* I299: decomposicao de caminho — estatisticas per-cpu no modulo,      */
/* nunca em `struct rq` (KMI intacto).                                  */

static DEFINE_PER_CPU(unsigned int, asi_placement_count);

static void asi_select_task_rq_handler(struct task_struct *p, int prev_cpu,
				       int __maybe_unused wake_flags,
				       int *new_cpu)
{
	struct sched_domain *sd;
	unsigned long best = 0;
	int cpu = prev_cpu;

	if (!asi_hook_enabled)
		return;				/* I296: Lindblad */

	if (asi_phi_of(p) < asi_phi_threshold)
		return;				/* I301: blindagem — CFS decide */

	/* I298: operador de fase cubica — desloca a tarefa ao nucleo de
	 * maxima capacidade do dominio LLC da CPU previa. */
	rcu_read_lock();
	sd = rcu_dereference(per_cpu(sd_llc, prev_cpu));
	if (sd) {
		int c;

		for_each_cpu_and(c, sched_domain_span(sd), cpu_online_mask) {
			unsigned long cap = arch_scale_cpu_capacity(c);

			if (cap > best) {
				best = cap;
				cpu = c;
			}
		}
	}
	rcu_read_unlock();

	*new_cpu = cpu;
	this_cpu_inc(asi_placement_count);
}

static void asi_task_should_scx_handler(struct task_struct *p,
					bool *use_scx)
{
	if (!asi_hook_enabled)
		return;				/* I296: Lindblad */

	/* I298: retem no dominio ASI as tarefas coerencialmente altas,
	 * impedindo a cessao ao sched_ext coexistente. */
	if (asi_phi_of(p) >= asi_phi_threshold)
		*use_scx = false;
}

/* ------------------------------------------------------------------ */
/* Interface /proc                                                 */

static int asi_proc_show(struct seq_file *m, void *v)
{
	unsigned int cpu;
	unsigned int total = 0;

	for_each_possible_cpu(cpu)
		total += per_cpu(asi_placement_count, cpu);

	seq_printf(m, "enabled:   %u\n", asi_hook_enabled);
	seq_printf(m, "threshold: %u/%u\n", asi_phi_threshold, ASI_PHI_SCALE);
	seq_puts(m, "per_cpu: ");
	for_each_possible_cpu(cpu)
		seq_printf(m, "%u ", per_cpu(asi_placement_count, cpu));
	seq_printf(m, "\ntotal: %u\n", total);
	return 0;
}

static ssize_t asi_proc_write(struct file *file, const char __user *buf,
			      size_t count, loff_t *ppos)
{
	char kbuf[32];
	unsigned int pid_v, phi_v;
	ssize_t n;

	if (count > sizeof(kbuf) - 1)
		return -EINVAL;
	n = simple_write_to_buffer(kbuf, sizeof(kbuf) - 1, ppos, buf, count);
	if (n <= 0)
		return n;
	kbuf[n] = '\0';

	if (sscanf(kbuf, "%u %u", &pid_v, &phi_v) != 2)
		return -EINVAL;

	asi_task_register((pid_t)pid_v, phi_v);
	return n;
}

static int asi_proc_open(struct inode *inode, struct file *file)
{
	return single_open(file, asi_proc_show, NULL);
}

static const struct proc_ops asi_proc_ops = {
	.proc_open	= asi_proc_open,
	.proc_read	= seq_read,
	.proc_write	= asi_proc_write,
	.proc_lseek	= seq_lseek,
	.proc_release	= single_release,
};

/* ------------------------------------------------------------------ */

static int __init asi_sched_module_init(void)
{
	proc_create("asi_sched_tasks", 0644, NULL, &asi_proc_ops);

	/* Hooks restritos: enquanto registrados, o modulo nao pode ser
	 * descarregado (semantica DECLARE_RESTRICTED_HOOK). */
	register_trace_android_rvh_asi_select_task_rq(
		asi_select_task_rq_handler, NULL);
	register_trace_android_rvh_asi_task_should_scx(
		asi_task_should_scx_handler, NULL);

	pr_info("asi_sched: loaded (I296-I301, threshold=%u/%u)\n",
		asi_phi_threshold, ASI_PHI_SCALE);
	return 0;
}

static void __exit asi_sched_module_exit(void)
{
	unregister_trace_android_rvh_asi_select_task_rq(
		asi_select_task_rq_handler, NULL);
	unregister_trace_android_rvh_asi_task_should_scx(
		asi_task_should_scx_handler, NULL);
	tracepoint_synchronize_unregister();
	remove_proc_entry("asi_sched_tasks", NULL);
	pr_info("asi_sched: unloaded\n");
}

module_init(asi_sched_module_init);
module_exit(asi_sched_module_exit);

MODULE_LICENSE("GPL");
MODULE_AUTHOR("Arquiteto-Omega");
MODULE_DESCRIPTION("ASI scheduler vendor hooks (Catedral OS v287.0)");